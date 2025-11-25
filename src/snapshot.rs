/// Snapshot output structure (stub for compatibility)
pub struct SnapshotOutput {
    pub output: Vec<u8>,
}

/// Create a runtime snapshot (stub - QuickJS doesn't need/support V8-style snapshots)
///
/// QuickJS has much faster cold start times than V8, so snapshots are not necessary.
/// This function exists only for API compatibility with the runner.
pub fn create_runtime_snapshot() -> Result<SnapshotOutput, String> {
    // QuickJS doesn't need snapshots - return empty
    Ok(SnapshotOutput { output: vec![] })
}
