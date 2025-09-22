use wasmtime::InstanceAllocationStrategy;

pub fn create_config() -> wasmtime::Config {
    let mut config = wasmtime::Config::new();
    config
        .allocation_strategy(InstanceAllocationStrategy::OnDemand)
        .async_support(true)
        .coredump_on_trap(true)
        .cranelift_opt_level(wasmtime::OptLevel::SpeedAndSize)
        .epoch_interruption(true)
        .guard_before_linear_memory(true)
        .memory_may_move(false)
        .wasm_multi_memory(false)
        .wasm_backtrace(true)
        .debug_info(true);

    config
}
