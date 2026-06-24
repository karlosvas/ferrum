use {
    crate::{
        assets::{self, Asset, Model, ModelDesc, TypeModel},
        config::config::FerrumConfig,
    },
    std::{
        collections::HashMap,
        marker::PhantomData,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
            mpsc::{self, Receiver, Sender},
        },
    },
    wgpu::{BindGroupLayout, Device, Queue},
};

/// Handle to a model spawned in the [`ModelStore`]; the forge metaphor: the
/// caller keeps the ingot's id while the bead melts in the background.
pub struct Ingot<T> {
    pub id: usize,
    _marker: PhantomData<T>,
}

/// Loading state of a model: requested (`Burning`), ready (`Molten`) or
/// discarded (`Ash`).
pub(crate) enum Bead<T> {
    Burning,
    Molten(T),
    #[allow(dead_code)]
    Ash,
}

/// Owns every model of the scene and the channel through which asynchronously
/// loaded models arrive.
pub struct ModelStore {
    static_models: HashMap<usize, Bead<Model>>,
    light_models: HashMap<usize, Bead<Model>>,
    next_id: AtomicUsize,
    sender: Sender<(usize, Model)>,
    receiver: Receiver<(usize, Model)>,
}

impl ModelStore {
    pub(crate) fn new() -> Self {
        let (sender, receiver) = mpsc::channel::<(usize, Model)>();
        Self {
            static_models: HashMap::new(),
            light_models: HashMap::new(),
            next_id: AtomicUsize::new(0),
            sender,
            receiver,
        }
    }

    /// Registers the model as `Burning` and launches its load. The loading is
    /// asynchronous, and the model arrives via the channel when it finishes
    /// (`Bead::Burning` until then). In native: thread + block_on; in wasm
    /// there are no threads or block_on, so it's queued in the browser's event
    /// loop with spawn_local (asset fetching is already async).
    pub(crate) fn spawn(
        &mut self,
        device: &Arc<Device>,
        queue: &Arc<Queue>,
        layout: &Arc<BindGroupLayout>,
        model_desc: ModelDesc,
        config: &FerrumConfig,
    ) -> Ingot<Model> {
        let id: usize = self.next_id.fetch_add(1, Ordering::SeqCst);

        match model_desc.kind {
            TypeModel::StaticObj => self.static_models.insert(id, Bead::Burning),
            TypeModel::PointOfLight => self.light_models.insert(id, Bead::Burning),
        };

        let device: Arc<Device> = Arc::clone(device);
        let queue: Arc<Queue> = Arc::clone(queue);
        let layout: Arc<BindGroupLayout> = Arc::clone(layout);
        let sender: Sender<(usize, Model)> = self.sender.clone();
        let instances: Vec<assets::Instance> = model_desc.instances;
        let kind: TypeModel = model_desc.kind;
        let file_name: String = model_desc.file_name.to_string();
        let asset: Asset = config.asset.clone();

        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || {
            let result: Result<Model, anyhow::Error> = pollster::block_on(assets::load_model(
                &asset, &file_name, &device, &queue, &layout, instances, kind,
            ));
            match result {
                Ok(model) => {
                    let _ = sender.send((id, model));
                }
                Err(e) => log::error!("Failed to load model '{file_name}': {e:?}"),
            }
        });

        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(async move {
            let result: Result<Model, anyhow::Error> = assets::load_model(
                &asset, &file_name, &device, &queue, &layout, instances, kind,
            )
            .await;
            match result {
                Ok(model) => {
                    let _ = sender.send((id, model));
                }
                Err(e) => log::error!("Failed to load model '{file_name}': {e:?}"),
            }
        });

        Ingot {
            id,
            _marker: PhantomData,
        }
    }

    /// Drains the channel and turns every finished load into `Molten`.
    pub(crate) fn collect_loaded(&mut self) {
        while let Ok((id, model)) = self.receiver.try_recv() {
            match model.type_model {
                TypeModel::StaticObj => self.static_models.insert(id, Bead::Molten(model)),
                TypeModel::PointOfLight => self.light_models.insert(id, Bead::Molten(model)),
            };
        }
    }

    pub(crate) fn static_loaded(&self) -> impl Iterator<Item = &Model> {
        self.static_models.values().filter_map(|bead| match bead {
            Bead::Molten(model) => Some(model),
            _ => None,
        })
    }

    pub(crate) fn light_loaded(&self) -> impl Iterator<Item = &Model> {
        self.light_models.values().filter_map(|bead| match bead {
            Bead::Molten(model) => Some(model),
            _ => None,
        })
    }

    pub(crate) fn light_model_mut(&mut self, id: &usize) -> Option<&mut Model> {
        match self.light_models.get_mut(id) {
            Some(Bead::Molten(model)) => Some(model),
            _ => None,
        }
    }
}
