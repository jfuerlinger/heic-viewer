//! HEIC → JPEG conversion used by the Explorer context menu entries and the command line.

mod batch;
mod convert;
mod exif;
pub mod text;

pub use batch::{default_workers, Batch, Outcome, Progress};
pub use convert::{candidate_name, convert_file, encode_jpeg, is_heic, ConvertError, JPEG_QUALITY};

/// Runs a closed batch to completion with the default worker pool.
pub fn convert_all<P: AsRef<std::path::Path>>(inputs: &[P]) -> std::sync::Arc<Batch> {
    let batch = Batch::new();
    batch.add(inputs);
    batch.close();
    for worker in batch.spawn_workers(default_workers()) {
        let _ = worker.join();
    }
    batch
}
