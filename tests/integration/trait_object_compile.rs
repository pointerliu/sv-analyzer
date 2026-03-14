use dac26_mcp::ast::AstProvider;
use dac26_mcp::block::Blockizer;
use dac26_mcp::coverage::CoverageTracker;
use dac26_mcp::slicer::Slicer;
use dac26_mcp::wave::WaveformReader;

fn accepts_traits(
    _ast: &dyn AstProvider,
    _blockizer: &dyn Blockizer,
    _coverage: &dyn CoverageTracker,
    _wave: &dyn WaveformReader,
    _slicer: &dyn Slicer,
) {
}

#[test]
fn trait_objects_compile() {
    let _ = accepts_traits;
}
