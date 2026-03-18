use dac26_core::ast::AstProvider;
use dac26_core::block::Blockizer;
use dac26_core::coverage::CoverageTracker;
use dac26_core::slicer::Slicer;
use dac26_core::wave::WaveformReader;

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
