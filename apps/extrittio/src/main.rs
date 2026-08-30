use anyhow::Result;

// Extrittio Edge deliberately uses the system allocator. Some current
// Raspberry Pi OS kernels use a 16 KiB page size, which jemalloc 0.6 does not
// support. Keep jemalloc enabled for the regular native server builds.
#[cfg(all(not(target_env = "msvc"), feature = "jemalloc"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[tokio::main]
async fn main() -> Result<()> {
    extrittio::run().await
}
