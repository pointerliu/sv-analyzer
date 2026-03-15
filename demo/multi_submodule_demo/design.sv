module submodule1(
  input  logic src,
  input  logic enable,
  output logic stage1
);
  logic src_masked;
  logic src_inverted;

  assign src_masked = src & enable;
  assign src_inverted = ~src;

  always_comb begin
    stage1 = src_masked | src_inverted;
  end
endmodule

module submodule2(
  input  logic clk,
  input  logic rst_n,
  input  logic stage1,
  output logic result
);
  logic stage2;

  always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
      stage2 <= 1'b0;
    end else begin
      stage2 <= stage1;
    end
  end

  assign result = stage2;
endmodule

module top(
  input  logic clk,
  input  logic rst_n,
  input  logic src,
  input  logic enable,
  output logic result
);
  logic stage1;

  submodule1 u_sub1(
    .src(src),
    .enable(enable),
    .stage1(stage1)
  );

  submodule2 u_sub2(
    .clk(clk),
    .rst_n(rst_n),
    .stage1(stage1),
    .result(result)
  );
endmodule
