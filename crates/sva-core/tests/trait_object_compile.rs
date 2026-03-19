use sva_core::ast::AstProvider;
use sva_core::block::Blockizer;
use sva_core::coverage::CoverageTracker;
use sva_core::slicer::Slicer;
use sva_core::wave::WaveformReader;

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
