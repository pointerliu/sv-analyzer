module tb;
  logic clk;
  logic rst_n;
  logic src;
  logic enable;
  logic result;

  top u_top(
    .clk(clk),
    .rst_n(rst_n),
    .src(src),
    .enable(enable),
    .result(result)
  );
endmodule
