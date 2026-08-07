use std::collections::HashMap;

use pyo3::{Bound, Py, PyResult, PyTraverseError, PyVisit, pyclass, pymethods};

use crate::anim::Anim;
use crate::model::Model;
use crate::utils::Dict;

#[pyclass]
pub struct Scene {
    pub nodes: Vec<Py<Node>>,
    pub models: Vec<Py<Model>>,
    pub anims: Vec<Anim>,
}

impl Scene {
    /// # Preconditions
    ///
    /// `parent` is assumed to belong in `Self`.
    fn create_node(slf: &Bound<Self>, parent: Option<Py<Node>>, name: String, extra: Dict) -> PyResult<Py<Node>> {
        let mut obj = slf.borrow_mut();
        let node = Py::new(
            slf.py(),
            Node {
                name,
                index: obj.nodes.len(),
                extra,
                parent,
                scene: slf.as_unbound().clone_ref(slf.py()),
            },
        )?;
        obj.nodes.push(node.clone_ref(slf.py()));
        Ok(node)
    }
}

#[pymethods]
impl Scene {
    #[new]
    fn __new__() -> Self {
        Self {
            nodes: Vec::new(),
            models: Vec::new(),
            anims: Vec::new(),
        }
    }

    #[pyo3(signature = (name, extra = Dict::default()))]
    fn create_root_node(slf: Bound<Self>, name: String, extra: Dict) -> PyResult<Py<Node>> {
        Scene::create_node(&slf, None, name, extra)
    }

    fn __traverse__(&self, visit: PyVisit) -> Result<(), PyTraverseError> {
        self.nodes.iter().try_for_each(|node| visit.call(node))?;
        self.models.iter().try_for_each(|model| visit.call(model))
        // self.anims.iter().try_for_each(|anim| visit.call(anim))?;
    }

    fn __clear__(&mut self) {
        self.nodes.clear();
        self.models.clear();
        // self.anims.clear();
    }
}

#[pyclass(frozen)]
pub struct Node {
    pub name: String,
    pub index: usize,
    pub extra: HashMap<String, String>,
    pub parent: Option<Py<Self>>,
    pub scene: Py<Scene>,
}

#[pymethods]
impl Node {
    fn create_child_node(slf: Bound<Self>, name: String, extra: Dict) -> PyResult<Py<Node>> {
        let parent = slf.as_unbound().clone_ref(slf.py());
        let scene = slf.get().scene.bind(slf.py());
        Scene::create_node(scene, Some(parent), name, extra)
    }

    fn __traverse__(&self, visit: PyVisit) -> Result<(), PyTraverseError> {
        visit.call(&self.parent)?;
        visit.call(&self.scene)
    }
}
