use std::collections::HashMap;

use pyo3::{Bound, Py, PyResult, PyTraverseError, PyVisit, pyclass, pymethods};

use crate::model::Model;
use crate::utils::Dict;

#[pyclass]
pub struct Scene {
    pub nodes: Vec<Py<Node>>,
    pub models: Vec<Py<Model>>,
}

#[pymethods]
impl Scene {
    #[new]
    fn __new__() -> Self {
        Self {
            nodes: Vec::new(),
            models: Vec::new(),
        }
    }

    #[pyo3(signature = (name, extra = None))]
    fn create_root_node(slf: Bound<Self>, name: String, extra: Option<Dict>) -> PyResult<Py<Node>> {
        create_node(&slf, None, name, extra.unwrap_or_default())
    }

    fn __traverse__(&self, visit: PyVisit) -> Result<(), PyTraverseError> {
        self.nodes.iter().try_for_each(|node| visit.call(node))?;
        self.models.iter().try_for_each(|model| visit.call(model))
    }

    fn __clear__(&mut self) {
        self.nodes.clear();
        self.models.clear();
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
    #[pyo3(signature = (name, extra = None))]
    fn create_child_node(slf: Bound<Self>, name: String, extra: Option<Dict>) -> PyResult<Py<Node>> {
        let parent = slf.as_unbound().clone_ref(slf.py());
        let scene = slf.get().scene.bind(slf.py());
        create_node(scene, Some(parent), name, extra.unwrap_or_default())
    }

    fn __traverse__(&self, visit: PyVisit) -> Result<(), PyTraverseError> {
        visit.call(&self.parent)?;
        visit.call(&self.scene)
    }
}

/// # Preconditions
///
/// `parent` is assumed to belong to `scene`.
fn create_node(scene: &Bound<Scene>, parent: Option<Py<Node>>, name: String, extra: Dict) -> PyResult<Py<Node>> {
    let mut obj = scene.borrow_mut();
    let node = Py::new(
        scene.py(),
        Node {
            name,
            index: obj.nodes.len(),
            extra,
            parent,
            scene: scene.as_unbound().clone_ref(scene.py()),
        },
    )?;
    obj.nodes.push(node.clone_ref(scene.py()));
    Ok(node)
}
