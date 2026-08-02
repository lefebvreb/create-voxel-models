use pyo3::{Bound, Py, PyResult, PyTraverseError, PyVisit, pyclass, pymethods};

use crate::anim::Anim;
use crate::model::Model;
use crate::palette::Palette;

#[pyclass]
pub(crate) struct Scene {
    palette: Py<Palette>,
    nodes: Vec<Py<Node>>,
    models: Vec<Py<Model>>,
    anims: Vec<Anim>,
}

#[pymethods]
impl Scene {
    #[new]
    fn __new__(palette: Py<Palette>) -> Self {
        Self {
            palette,
            nodes: Vec::new(),
            models: Vec::new(),
            anims: Vec::new(),
        }
    }

    fn create_node(slf: Bound<Self>) -> PyResult<Py<Node>> {
        let mut obj = slf.borrow_mut();
        let node = Py::new(
            slf.py(),
            Node {
                index: obj.nodes.len(),
                scene: slf.as_unbound().clone_ref(slf.py()),
            },
        )?;
        obj.nodes.push(node.clone_ref(slf.py()));
        Ok(node)
    }

    fn create_model(slf: Bound<Self>, parent: Py<Node>, dimensions: (u8, u8, u8)) -> PyResult<Py<Model>> {
        let model = Model::new();
        todo!()
    }

    fn __traverse__(&self, visit: PyVisit) -> Result<(), PyTraverseError> {
        visit.call(&self.palette)?;
        self.nodes.iter().try_for_each(|node| visit.call(node))?;
        self.models.iter().try_for_each(|model| visit.call(model))?;
        // self.anims.iter().try_for_each(|anim| visit.call(anim))?;
        Ok(())
    }

    fn __clear__(&mut self) {
        self.nodes.clear();
        self.models.clear();
        // self.anims.clear();
    }
}

#[pyclass]
pub(crate) struct Node {
    index: usize,
    scene: Py<Scene>,
}

#[pymethods]
impl Node {
    fn __traverse__(&self, visit: PyVisit) -> Result<(), PyTraverseError> {
        visit.call(&self.scene)
    }
}
