This is a dataflow analysis tool designed for verilog/systemverilog code, written in rust.

- use sv-parser to parse AST
- use wellen to parse waveform
- this tool should support for static dataflow analysis & dynamic dataflow analysis
- static analysis given a set of variables, collect all statements that will affact these variable values;, result is unorderd set;
- dynamic dataflow analysis is the Blues alogrim at @dac26.pdf
- analysis should be perform on both statment level and block level, first analyze on statment level, then group to block level
- blockization approach should use same with @dac26.pdf
- verilator support `--trace-coverage` to record statements coverage in vcd waveform, there is a demo to get coverage @demo folder, check it.
- i have a prototype implemendted by myself, but it parse coverage not use `--trace-coverage`, and the code style is bad, so you should implemented from scratch, but if met some problems, you can reference my implementation (e.g., crate API usage), @sv-analysis
- @dac26.pdf and @sv-analysis has some component about llm related, we don't care them for now, we only focus on dataflow analysis at block level. the final result should be a graph. if at statment level, nodes are (s, t), where s is a statment that driven a set of left-values, t is time annnotation. edges from (s2, t2) -> (s1, t1) means that the input variables of s1 at time t1 are affacted by s2's output variables at t2. here are some input and output variables definition:
  - a <= b + c: `a` is output variable, `b, c` are input variables
  - if (a) b <= c;  `b` is output variabls, `a, c` are input variables.
  - case (a) 2'b01: b <= c, `b` is output variabls, `a, c` are input variables.
  - more definiation can be foound @dac26.pdf
 

