---
name: rmcp-usage
description: Rust SDK for Model Context Protocol (MCP) - build MCP servers and clients with tokio async runtime
---

# rmcp

Rust SDK for Model Context Protocol (MCP) with tokio async runtime. Build MCP servers and clients that communicate over stdio, SSE, HTTP, or child processes.

## Quick Reference

| Need | Solution |
|------|----------|
| Build an MCP server | Implement `ServerHandler` trait + use `#[tool]` macros |
| Run server over stdio | `MyServer::new().serve(stdio()).await?` |
| Build an MCP client | Use `()` or `ClientInfo` + `serve_client()` |
| Connect to child process server | `TokioChildProcess::new(Command::new("npx").arg("-y").arg("@modelcontextprotocol/server-everything"))` |
| Expose a tool | `#[tool]` macro on async/sync functions |
| Return tool result | `CallToolResult::success(vec![Content::text("...")])` |

## Installation

```toml
rmcp = { version = "1.2", features = ["server"] }  # for servers
rmcp = { version = "1.2", features = ["client"] }  # for clients
rmcp = { version = "1.2", features = ["server", "client", "transport-sse", "transport-child-process"] } # full features
```

## Feature Flags

| Feature | Enables |
|---------|---------|
| `server` | Server-side SDK (implies `transport-async-rw`) |
| `client` | Client-side SDK |
| `macros` | `#[tool]` and `#[tool_box]` macros (default) |
| `transport-io` | Stdio transport for servers |
| `transport-child-process` | Child process transport for clients |
| `transport-sse` | SSE client transport |
| `transport-sse-server` | SSE server transport |
| `base64` | Image encoding support |

## Core Traits

### `ServerHandler` - Implement your MCP server

```rust
use rmcp::{ServerHandler, service::RequestContext, model::{ServerInfo, ServerCapabilities, CallToolResult, Content}};
use rmcp::handler::server::tool::schema_for_type;

pub trait ServerHandler: Send + Sync + Clone + 'static {
    fn get_info(&self) -> ServerInfo;  // Server capabilities + info
    
    async fn list_tools(&self, ...) -> Result<ListToolsResult, McpError>;
    async fn call_tool(&self, ...) -> Result<CallToolResult, McpError>;
    
    // Resources and prompts also overridable...
}
```

### `ClientHandler` - Implement your MCP client

```rust
pub trait ClientHandler: Send + Sync + 'static {
    fn ping(&self, context: RequestContext<RoleClient>) -> ...;
    fn create_message(&self, ...) -> ...;
    fn list_roots(&self, ...) -> ...;
}
```

### `ServiceExt` - Bridge handler to transport

```rust
impl<H: ServerHandler> ServiceExt<RoleServer> for H {
    fn serve<T, E, A>(self, transport: T) -> impl Future<Output = Result<RunningService<RoleServer, Self>, E>>;
}
```

## Minimal Server Example

```rust
use rmcp::{ServerHandler, ServiceExt, transport::stdio, model::{ServerInfo, ServerCapabilities, CallToolResult, Content, ListToolsResult, Tool}};

#[derive(Clone)]
struct MyServer;

impl ServerHandler for MyServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("My MCP server".into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }

    async fn list_tools(&self, _: Option<PaginatedRequestParam>, _: RequestContext<RoleServer>) 
        -> Result<ListToolsResult, McpError> 
    {
        Ok(ListToolsResult {
            next_cursor: None,
            tools: vec![Tool {
                name: "hello".into(),
                description: Some("Say hello".into()),
                input_schema: serde_json::json!({"type": "object", "properties": {"name": {"type": "string"}}}),
                annotations: None,
            }],
        })
    }

    async fn call_tool(&self, request: CallToolRequestParam, _: RequestContext<RoleServer>) 
        -> Result<CallToolResult, McpError> 
    {
        let name = request.arguments.as_ref()
            .and_then(|args| args.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("World");
        Ok(CallToolResult::success(vec![Content::text(format!("Hello, {name}!"))]))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    MyServer.serve(stdio()).await?.waiting().await?;
    Ok(())
}
```

## Tool Macros - `#[tool]` and `#[tool_box]`

Use macros for ergonomic tool definition with automatic schema generation:

```rust
use rmcp::{ServerHandler, model::{ServerInfo, ServerCapabilities}, schemars, tool};
use rmcp::handler::server::tool::schema_for_type;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SumRequest {
    #[schemars(description = "first number")]
    pub a: i32,
    #[schemars(description = "second number")]
    pub b: i32,
}

#[derive(Debug, Clone)]
pub struct Calculator;

#[tool(tool_box)]
impl Calculator {
    #[tool(description = "Add two numbers", output_schema = "schema_for_type::<String>()")]
    fn sum(&self, #[tool(aggr)] SumRequest { a, b }: SumRequest) -> String {
        (a + b).to_string()
    }

    #[tool(description = "Subtract two numbers")]
    fn sub(&self, #[tool(param)] a: i32, #[tool(param)] b: i32) -> String {
        (a - b).to_string()
    }
}

#[tool(tool_box)]
impl ServerHandler for Calculator {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("Calculator".into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
```

### Macro Attributes

| Attribute | Purpose |
|-----------|---------|
| `#[tool_box]` | Applied to impl block - creates static toolbox for `#[tool]` methods |
| `#[tool(name = "...", description = "...", output_schema = ...)]` | Marks a method as a tool |
| `#[tool(param)]` | Extract individual parameter from JSON args |
| `#[tool(aggr)]` | Aggregate all parameters into a struct |
| `#[tool(callee)]` | Pass the service instance |
| `#[tool(ct)]` | Pass the cancellation token |

## Content Types

```rust
use rmcp::model::{Content, RawContent, CallToolResult};

// Text content
Content::text("Hello!")
RawContent::text("Hello!")

// Image content (base64 encoded)
Content::image(base64_data, "image/png")

// JSON content
Content::json(serde_json::json!({"key": "value"}))

// Error result
CallToolResult::error(vec![Content::text("Something went wrong")])
```

## Client Usage

### Connect to a child process server

```rust
use rmcp::{ServiceExt, transport::TokioChildProcess, model::CallToolRequestParam};
use tokio::process::Command;

let service = ()
    .serve(TokioChildProcess::new(
        Command::new("npx").arg("-y").arg("@modelcontextprotocol/server-everything")
    )?)
    .await?;

let tools = service.list_all_tools().await?;
let result = service.call_tool(CallToolRequestParam {
    name: "echo".into(),
    arguments: Some(object!({ "message": "hi" })),
}).await?;
```

### Stdio client (for testing)

```rust
let service = ().serve((tokio::io::stdin(), tokio::io::stdout())).await?;
```

## Transport Types

| Transport | Use Case | Import |
|-----------|----------|--------|
| `stdio()` | Server stdio | `rmcp::transport::stdio` |
| `TokioChildProcess` | Client to child process | `rmcp::transport::TokioChildProcess` |
| `(stdin, stdout)` tuple | Manual stdio | `(tokio::io::stdin(), tokio::io::stdout())` |
| `(read, write)` tuple | Any async read/write | `(reader, writer)` |
| `SseTransport` | Client over SSE | `rmcp::transport::SseTransport` |
| `SseServer` | Server over SSE | `rmcp::transport::SseServer` |

## Server Capabilities

```rust
use rmcp::model::ServerCapabilities;

ServerCapabilities::builder()
    .enable_tools()                    // Expose tools
    .enable_tool_list_changed()        // Dynamic tool updates
    .enable_resources()                // Expose resources
    .enable_resource_list_changed()    // Dynamic resource updates
    .enable_prompts()                  // Expose prompts
    .enable_prompt_list_changed()      // Dynamic prompt updates
    .enable_logging()                  // Accept logging messages
    .enable_experimental()             // Experimental features
    .build()
```

## RunningService

```rust
let server = MyServer.serve(stdio()).await?;

// Get peer to send requests/notifications
let peer = server.peer();

// Wait for shutdown
let reason = server.waiting().await?;

// Cancel explicitly
let reason = server.cancel().await?;
```

## Common Patterns

### Resources

```rust
async fn list_resources(&self, _: Option<PaginatedRequestParam>, _: RequestContext<RoleServer>) 
    -> Result<ListResourcesResult, McpError> 
{
    Ok(ListResourcesResult {
        resources: vec![
            Resource {
                uri: "my://resource".into(),
                name: Some("My Resource".into()),
                mime_type: Some("text/plain".into()),
                description: Some("A test resource".into()),
            }
        ],
        next_cursor: None,
    })
}

async fn read_resource(&self, request: ReadResourceRequestParam, _: RequestContext<RoleServer>) 
    -> Result<ReadResourceResult, McpError> 
{
    if request.uri == "my://resource" {
        Ok(ReadResourceResult {
            contents: vec![ResourceContents::TextResourceContents {
                uri: "my://resource".into(),
                mime_type: Some("text/plain".into()),
                text: "Resource content".into(),
            }],
        })
    } else {
        Err(McpError::resource_not_found("Resource not found", None))
    }
}
```

### Prompts

```rust
async fn list_prompts(&self, _: Option<PaginatedRequestParam>, _: RequestContext<RoleServer>) 
    -> Result<ListPromptsResult, McpError> 
{
    Ok(ListPromptsResult {
        prompts: vec![
            Prompt {
                name: "greeting".into(),
                description: Some("Generate a greeting".into()),
                arguments: Some(vec![
                    PromptArgument {
                        name: "name".into(),
                        description: Some("Name to greet".into()),
                        required: true,
                    }
                ]),
            }
        ],
    })
}

async fn get_prompt(&self, request: GetPromptRequestParam, _: RequestContext<RoleServer>) 
    -> Result<GetPromptResult, McpError> 
{
    if request.name == "greeting" {
        let name = request.arguments.as_ref()
            .and_then(|m| m.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("World");
        Ok(GetPromptResult {
            description: Some("A friendly greeting".into()),
            messages: vec![
                PromptMessage {
                    role: Role::User,
                    content: Content::text(format!("Hello, {name}!")),
                }
            ],
        })
    } else {
        Err(McpError::resource_not_found("Prompt not found", None))
    }
}
```

## Error Handling

```rust
use rmcp::Error;

impl IntoCallToolResult for () {
    fn into_call_tool_result(self) -> Result<CallToolResult, Error> {
        Ok(CallToolResult::success(vec![]))
    }
}

// For Result<T, E> where both implement IntoContents
impl<T: IntoContents, E: IntoContents> IntoCallToolResult for Result<T, E> {
    fn into_call_tool_result(self) -> Result<CallToolResult, Error> {
        match self {
            Ok(v) => Ok(CallToolResult::success(v.into_contents())),
            Err(e) => Ok(CallToolResult::error(e.into_contents())),
        }
    }
}
```

## Dependency Injection in Tools

```rust
#[tool(tool_box)]
impl MyService {
    #[tool]
    async fn do_something(
        &self,                              // &Self from #[tool(callee)] or implicit
        #[tool(ct)] ct: CancellationToken,  // Request cancellation token
        #[tool(param)] name: String,        // Individual param
        #[tool(aggr)] params: MyParams,    // All params as struct
    ) -> Result<CallToolResult, Error> {
        Ok(CallToolResult::success(vec![Content::text("done")]))
    }
}
```

## Dynamic Service Collection

```rust
// Combine multiple handlers into one
let combined: Box<dyn DynService<RoleServer>> = handler1.into_dyn();
let combined = service.into_dyn();
```

## Logging Setup

```rust
use tracing_subscriber::{self, EnvFilter};

tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::DEBUG.into()))
    .with_writer(std::io::stderr)
    .with_ansi(false)
    .init();
```

## Key Types Summary

| Type | Purpose |
|------|---------|
| `ServerHandler` | Trait for implementing MCP server logic |
| `ClientHandler` | Trait for implementing MCP client logic |
| `RunningService<R, S>` | Active service with peer access |
| `Peer<R>` | Send requests/notifications to remote |
| `RequestContext<R>` | Per-request context with cancellation |
| `CallToolResult` | Result from a tool call |
| `Content` | Text, image, or embedded resource content |
| `ServerCapabilities` | Declare what server supports |
| `Tool` | Tool definition with name, description, schema |
| `Resource` | Resource definition |
| `Prompt` | Prompt template definition |

## Gotchas / Anti-patterns

- **Don't forget `..Default::default()`** when building `ServerInfo` - fields like `protocol_version` and `server_info` must be set
- **Return type inference** - the `#[tool]` macro requires explicit `output_schema` or relies on return type inference via `IntoCallToolResult`
- **Async vs sync tools** - both work, but async tools use `IntoCallToolResultFut` wrapper
- **Cancellation** - `RequestContext` has a `ct` token that cancels when client sends `CancelledNotification`
- **Tool not found** - return `Error::invalid_params("tool not found", None)` rather than panicking

## Further Reading

- [crates.io](https://crates.io/crates/rmcp)
- [docs.rs](https://docs.rs/rmcp)
- [GitHub](https://github.com/modelcontextprotocol/rust-sdk)
- [MCP Specification](https://spec.modelcontextprotocol.io/)
