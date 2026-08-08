use pyo3::{Bound, Py, PyResult, PyTraverseError, PyVisit, pyclass, pymethods};

use crate::math::{Quat, Vec3};
use crate::model::Model;
use crate::utils::Dict;

#[pyclass]
pub struct Scene {
    pub nodes: Vec<Py<Node>>,
    pub models: Vec<NodeModel>,
}

impl Scene {
    fn add_node(slf: &Bound<Self>, f: impl FnOnce(usize) -> Node) -> PyResult<Py<Node>> {
        let mut slf_brw = slf.borrow_mut();
        let node = Py::new(slf.py(), f(slf_brw.nodes.len()))?;
        slf_brw.nodes.push(node.clone_ref(slf.py()));
        Ok(node)
    }

    fn add_model(slf: &Bound<Self>, f: impl FnOnce(usize) -> NodeModel) -> PyResult<()> {
        let mut slf_brw = slf.borrow_mut();
        let model = f(slf_brw.nodes.len());
        slf_brw.models.push(model);
        Ok(())
    }
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

    #[pyo3(signature = (name, *, extra = None))]
    fn create_root_node(slf: Bound<Self>, name: String, extra: Option<Dict>) -> PyResult<Py<Node>> {
        Self::add_node(&slf, |index| Node { 
            name,
            index,
            translation: None,
            rotation: None,
            scale: None,
            extra: extra.unwrap_or_default(),
            parent: None,
            scene: slf.clone().unbind(),
        })
    }

    fn __traverse__(&self, visit: PyVisit) -> Result<(), PyTraverseError> {
        self.nodes.iter().try_for_each(|node| visit.call(node))?;
        self.models.iter().try_for_each(|model| model.__traverse__(&visit))
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
    pub translation: Option<Vec3>,
    pub rotation: Option<Quat>,
    pub scale: Option<Vec3>,
    pub extra: Dict,
    #[pyo3(get)]
    pub parent: Option<Py<Node>>,
    #[pyo3(get)]
    pub scene: Py<Scene>,
}

#[pymethods]
impl Node {
    #[pyo3(signature = (name, *, translation = None, rotation = None, scale = None, extra = None))]
    fn create_child_node(
        slf: Bound<Self>,
        name: String,
        translation: Option<Vec3>,
        rotation: Option<Quat>,
        scale: Option<Vec3>,
        extra: Option<Dict>,
    ) -> PyResult<Py<Node>> {
        let scene = slf.get().scene.bind(slf.py());
        Scene::add_node(scene, |index| Node {
            name,
            index,
            translation,
            rotation,
            scale,
            extra: extra.unwrap_or_default(),
            parent: Some(slf.clone().unbind()),
            scene: scene.clone().unbind(),
        })
    }

    #[pyo3(signature = (name, model, *, extra = None))]
    fn add_model(slf: Bound<Self>, name: String, model: Py<Model>, extra: Option<Dict>) -> PyResult<()> {
        let slf_brw = slf.get();
        let scene = slf_brw.scene.bind(slf.py());
        Scene::add_model(scene, |index| NodeModel {
            name,
            index,
            extra: extra.unwrap_or_default(),
            parent: slf.clone().unbind(),
            model,
            scene: scene.clone().unbind(),
        })
    }

    fn __traverse__(&self, visit: PyVisit) -> Result<(), PyTraverseError> {
        visit.call(&self.parent)?;
        visit.call(&self.scene)
    }
}

pub struct NodeModel {
    pub name: String,
    pub index: usize,
    pub extra: Dict,
    pub parent: Py<Node>,
    pub model: Py<Model>,
    pub scene: Py<Scene>,
}

impl NodeModel {
    fn __traverse__(&self, visit: &PyVisit) -> Result<(), PyTraverseError> {
        visit.call(&self.parent)?;
        visit.call(&self.model)?;
        visit.call(&self.scene)
    }
}
