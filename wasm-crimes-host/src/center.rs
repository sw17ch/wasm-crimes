use std::{alloc::Layout, path::Path, ptr::NonNull, sync::Arc, time::Duration};

use wasmtime::{
    Config, Engine, InstancePre, Linker, Memory, Module, Store, TypedFunc, UpdateDeadline,
};
use wasmtime_wasi::preview1;

use crate::{context::CrimeCtx, host_calls};

#[derive(thiserror::Error, Debug)]
pub enum CrimeSetupError {
    #[error("Error starting engine: {0}")]
    Engine(wasmtime::Error),
    #[error("Error loading module: {0}")]
    Module(wasmtime::Error),
    #[error("Error linking wasi preview1: {0}")]
    WasiP1Linker(wasmtime::Error),
    #[error("Error linking host calls: {0}")]
    HostLinker(wasmtime::Error),
    #[error("Error creating pre instance: {0}")]
    PreInstance(wasmtime::Error),
}

#[derive(thiserror::Error, Debug)]
pub enum CrimeInstantiateError {
    #[error("Error creating instance: {0}")]
    Instance(wasmtime::Error),
    #[error("Error getting module entry function '{0}': {1}")]
    FuncFailed(&'static str, wasmtime::Error),
    #[error("Error getting module memory")]
    Memory,
}

pub struct CrimeCenter {
    engine: Engine,
    instance_pre: InstancePre<CrimeCtx>,
    _ticker_handle: std::thread::JoinHandle<()>,
}

impl CrimeCenter {
    pub fn new(module_path: impl AsRef<Path>, config: Config) -> Result<Self, CrimeSetupError> {
        let engine = Engine::new(&config).map_err(CrimeSetupError::Engine)?;
        let module = Module::from_file(&engine, module_path).map_err(CrimeSetupError::Module)?;

        let mut linker: Linker<CrimeCtx> = Linker::new(&engine);
        preview1::add_to_linker_async(&mut linker, |t| t.wasi_ctx_mut())
            .map_err(CrimeSetupError::WasiP1Linker)?;
        host_calls::add_to_linker(&mut linker).map_err(CrimeSetupError::HostLinker)?;

        let instance_pre = linker
            .instantiate_pre(&module)
            .map_err(CrimeSetupError::PreInstance)?;

        let _ticker_handle = {
            let weak_engine = engine.weak();
            std::thread::spawn(move || {
                while let Some(engine) = weak_engine.upgrade() {
                    std::thread::sleep(Duration::from_millis(10));
                    engine.increment_epoch();
                }
            })
        };

        Ok(Self {
            engine,
            instance_pre,
            _ticker_handle,
        })
    }

    pub async fn instance(&self) -> Result<CrimeInstance, CrimeInstantiateError> {
        let inst = CrimeInstance::new(self).await?;
        Ok(inst)
    }
}

struct InnerCrime {
    store: Store<CrimeCtx>,
    memory: Memory,
    enter_mut_func: TypedFunc<(u32, u32), (u32,)>,
    alloc_func: TypedFunc<(u32, u32), (u32,)>,
    dealloc_func: TypedFunc<(u32, u32, u32), ()>,
    realloc_func: TypedFunc<(u32, u32, u32, u32), (u32,)>,
}

impl InnerCrime {
    async fn call_enter(&mut self, buf: &GuestBuffer, len: u32) -> wasmtime::Result<u32> {
        let (r,) = self
            .enter_mut_func
            .call_async(&mut self.store, (buf.offset, len))
            .await?;
        Ok(r)
    }

    async fn call_alloc(&mut self, layout: Layout) -> wasmtime::Result<Option<(NonNull<u8>, u32)>> {
        let Ok(size) = layout.size().try_into() else {
            panic!("size does not fit in a u32");
        };
        let Ok(align) = layout.align().try_into() else {
            panic!("align does not fit in a u32");
        };
        let (offset,) = self
            .alloc_func
            .call_async(&mut self.store, (size, align))
            .await?;
        if offset == 0 {
            Ok(None)
        } else {
            let ptr = self.memory.data_ptr(&mut self.store);
            let ptr = unsafe { ptr.add(offset as usize) };
            let ptr = unsafe { NonNull::new_unchecked(ptr) };
            Ok(Some((ptr, offset)))
        }
    }

    async fn call_dealloc(&mut self, buf: GuestBuffer) -> wasmtime::Result<()> {
        let Ok(size) = buf.layout.size().try_into() else {
            panic!("size does not fit in a u32");
        };
        let Ok(align) = buf.layout.align().try_into() else {
            panic!("align does not fit in a u32");
        };
        let () = self
            .dealloc_func
            .call_async(&mut self.store, (buf.offset, size, align))
            .await?;
        Ok(())
    }

    async fn call_realloc(
        &mut self,
        buf: GuestBuffer,
        new_layout: Layout,
    ) -> wasmtime::Result<Result<(NonNull<u8>, u32), GuestBuffer>> {
        assert_eq!(buf.layout.align(), new_layout.align());
        let Ok(old_size) = buf.layout.size().try_into() else {
            panic!("size does not fit in a u32");
        };
        let Ok(align) = buf.layout.align().try_into() else {
            panic!("align does not fit in a u32");
        };
        let Ok(new_size) = new_layout.size().try_into() else {
            panic!("new size does not fit into a u32");
        };
        let (offset,) = self
            .realloc_func
            .call_async(&mut self.store, (buf.offset, old_size, align, new_size))
            .await?;

        if offset == 0 {
            Ok(Err(buf))
        } else {
            let ptr = self.memory.data_ptr(&mut self.store);
            let ptr = unsafe { ptr.add(offset as usize) };
            let ptr = unsafe { NonNull::new_unchecked(ptr) };
            Ok(Ok((ptr, offset)))
        }
    }
}

pub struct CrimeInstance {
    inner: Arc<tokio::sync::Mutex<InnerCrime>>,
}

impl CrimeInstance {
    async fn new(center: &CrimeCenter) -> Result<Self, CrimeInstantiateError> {
        let ctx = CrimeCtx::new();
        let mut store = Store::new(&center.engine, ctx);
        store.epoch_deadline_callback(|_ctx| Ok(UpdateDeadline::Continue(1)));

        let instance = center
            .instance_pre
            .instantiate_async(&mut store)
            .await
            .map_err(CrimeInstantiateError::Instance)?;

        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or(CrimeInstantiateError::Memory)?;

        let enter_mut_func = instance
            .get_typed_func::<(u32, u32), (u32,)>(&mut store, "enter_mut")
            .map_err(|e| CrimeInstantiateError::FuncFailed("enter_mut", e))?;
        let alloc_func = instance
            .get_typed_func::<(u32, u32), (u32,)>(&mut store, "guest_alloc")
            .map_err(|e| CrimeInstantiateError::FuncFailed("guest_alloc", e))?;
        let dealloc_func = instance
            .get_typed_func::<(u32, u32, u32), ()>(&mut store, "guest_dealloc")
            .map_err(|e| CrimeInstantiateError::FuncFailed("guest_dealloc", e))?;
        let realloc_func = instance
            .get_typed_func::<(u32, u32, u32, u32), (u32,)>(&mut store, "guest_realloc")
            .map_err(|e| CrimeInstantiateError::FuncFailed("guest_realloc", e))?;

        Ok(CrimeInstance {
            inner: Arc::new(tokio::sync::Mutex::new(InnerCrime {
                store,
                memory,
                enter_mut_func,
                alloc_func,
                dealloc_func,
                realloc_func,
            })),
        })
    }

    pub async fn call_enter(&mut self, buf: &GuestBuffer, len: u32) -> wasmtime::Result<u32> {
        let mut inner = self.inner.lock().await;
        inner.call_enter(buf, len).await
    }

    pub async fn call_alloc(&mut self, layout: Layout) -> wasmtime::Result<Option<GuestBuffer>> {
        let mut inner = self.inner.lock().await;
        let Some((ptr, offset)) = inner.call_alloc(layout).await? else {
            return Ok(None);
        };
        Ok(Some(GuestBuffer {
            _inner: self.inner.clone(),
            layout,
            offset,
            ptr,
        }))
    }

    pub async fn call_dealloc(&mut self, buf: GuestBuffer) -> wasmtime::Result<()> {
        let mut inner = self.inner.lock().await;
        inner.call_dealloc(buf).await
    }

    pub async fn call_realloc(
        &mut self,
        buf: GuestBuffer,
        new_layout: Layout,
    ) -> wasmtime::Result<Result<GuestBuffer, GuestBuffer>> {
        let mut inner = self.inner.lock().await;
        Ok(match inner.call_realloc(buf, new_layout).await? {
            Ok((ptr, offset)) => Ok(GuestBuffer {
                _inner: self.inner.clone(),
                layout: new_layout,
                offset,
                ptr,
            }),
            Err(buf) => Err(buf),
        })
    }
}

pub struct GuestBuffer {
    /// we hold this to keep the inner pieces from dropping prematurely
    _inner: Arc<tokio::sync::Mutex<InnerCrime>>,
    /// layout of the guest allocation
    layout: Layout,
    /// offset _inside the memory_.
    offset: u32,
    /// the pointer to the buffer inside the guest. we keep the `inner` field
    /// above to ensure that this remains valid (as long as the memory isn't
    /// dropped).
    ptr: NonNull<u8>,
}

impl std::fmt::Debug for GuestBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GuestBuffer")
            .field("layout", &self.layout)
            .field("offset", &self.offset)
            .field("ptr", &self.ptr)
            .finish()
    }
}

impl GuestBuffer {
    /// Returns the slice as a mutable pointer.
    pub async unsafe fn as_mut(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.layout.size()) }
    }
}
