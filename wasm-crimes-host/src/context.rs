use wasmtime_wasi::{WasiCtxBuilder, preview1::WasiP1Ctx};

pub struct CrimeCtx {
    wasi_ctx: WasiP1Ctx,
    slot: u32,
}

impl CrimeCtx {
    pub fn new() -> Self {
        let wasi_ctx = WasiCtxBuilder::new()
            .inherit_stdio()
            .inherit_stderr()
            .inherit_env()
            .build_p1();
        Self { wasi_ctx, slot: 0 }
    }

    pub fn wasi_ctx_mut(&mut self) -> &mut WasiP1Ctx {
        &mut self.wasi_ctx
    }

    pub fn get(&self) -> u32 {
        self.slot
    }

    pub fn put(&mut self, v: u32) -> u32 {
        let r = self.slot;
        self.slot = v;
        r
    }
}
