use std::collections::HashMap;
use std::fs::write;
use std::path::PathBuf;

use pyo3::exceptions::PyValueError;
use pyo3::{Bound, Py, PyResult, PyTraverseError, PyVisit, pyclass, pymethods};

use crate::anim::Anim;
use crate::math::{Quat, Vec3};
use crate::model::Model;
use crate::render::{CameraAngle, RenderOutput};
use crate::tools::{export_glb, render};
use crate::utils::Dict;

#[pyclass]
#[derive(Default)]
pub struct Scene {
    pub anims: HashMap<String, Py<Anim>>,
    pub nodes: HashMap<String, Py<Node>>,
    pub meshes: HashMap<String, Py<Mesh>>,
}

impl Scene {
    fn add_node(slf: &Bound<Self>, node: Node) -> PyResult<Py<Node>> {
        let mut slf_brw = slf.borrow_mut();
        if slf_brw.nodes.contains_key(&node.name) {
            return Err(PyValueError::new_err(
                "there is already a node with this name, and duplicates are disallowed",
            ));
        }
        let name = node.name.clone();
        let node = Py::new(slf.py(), node)?;
        slf_brw.nodes.insert(name, node.clone_ref(slf.py()));
        Ok(node)
    }
}

#[pymethods]
impl Scene {
    #[new]
    pub fn __new__() -> Self {
        Self::default()
    }

    #[pyo3(signature = (name, *, extras = None))]
    pub fn create_root_node(slf: Bound<Self>, name: String, extras: Option<Dict>) -> PyResult<Py<Node>> {
        Self::add_node(
            &slf,
            Node {
                name,
                translation: None,
                rotation: None,
                scale: None,
                extras,
                parent: None,
                scene: slf.clone().unbind(),
            },
        )
    }

    #[pyo3(signature = (name, *, extras = None))]
    pub fn create_anim(slf: Bound<Self>, name: String, extras: Option<Dict>) -> PyResult<Py<Anim>> {
        let mut slf_brw = slf.borrow_mut();
        if slf_brw.anims.contains_key(&name) {
            return Err(PyValueError::new_err(
                "there is already an animation with this name, and duplicates are disallowed",
            ));
        }
        let anim = Py::new(
            slf.py(),
            Anim {
                name: name.clone(),
                extras,
                nodes: HashMap::default(),
                scene: slf.clone().unbind(),
            },
        )?;
        slf_brw.anims.insert(name, anim.clone_ref(slf.py()));
        Ok(anim)
    }

    pub fn export_glb(slf: Bound<Self>, path: PathBuf) -> PyResult<()> {
        let blob = export_glb(slf)?;
        write(path, blob)?;
        Ok(())
    }

    #[pyo3(signature = (angles, *, times = vec![], animation = None, include = None, exclude = None))]
    pub fn render(
        slf: Bound<Self>,
        angles: Vec<CameraAngle>,
        times: Vec<f64>,
        animation: Option<String>,
        include: Option<Vec<String>>,
        exclude: Option<Vec<String>>,
    ) -> PyResult<RenderOutput> {
        render(slf, angles, times, animation, include, exclude)
    }

    fn __traverse__(&self, visit: PyVisit) -> Result<(), PyTraverseError> {
        self.anims.values().try_for_each(|anim| visit.call(anim))?;
        self.nodes.values().try_for_each(|node| visit.call(node))?;
        self.meshes.values().try_for_each(|model| visit.call(model))
    }

    fn __clear__(&mut self) {
        self.anims.clear();
        self.nodes.clear();
        self.meshes.clear();
    }
}

#[pyclass(get_all, frozen)]
pub struct Node {
    pub name: String,
    pub translation: Option<Vec3>,
    pub rotation: Option<Quat>,
    pub scale: Option<Vec3>,
    pub extras: Option<Dict>,
    pub parent: Option<Py<Node>>,
    pub scene: Py<Scene>,
}

#[pymethods]
impl Node {
    #[pyo3(signature = (name, *, translation = None, rotation = None, scale = None, extras = None))]
    pub fn create_child_node(
        slf: Bound<Self>,
        name: String,
        translation: Option<Vec3>,
        rotation: Option<Quat>,
        scale: Option<Vec3>,
        extras: Option<Dict>,
    ) -> PyResult<Py<Node>> {
        let scene = slf.get().scene.bind(slf.py());
        Scene::add_node(
            scene,
            Node {
                name,
                translation,
                rotation,
                scale,
                extras,
                parent: Some(slf.clone().unbind()),
                scene: scene.clone().unbind(),
            },
        )
    }

    #[pyo3(signature = (name, model, *, extras = None))]
    pub fn add_model(slf: Bound<Self>, name: String, model: Py<Model>, extras: Option<Dict>) -> PyResult<Py<Mesh>> {
        let mut scene = slf.get().scene.borrow_mut(slf.py());
        if scene.meshes.contains_key(&name) {
            return Err(PyValueError::new_err(
                "there is already a mesh with this name, and duplicates are disallowed",
            ));
        }
        let mesh = Py::new(
            slf.py(),
            Mesh {
                name: name.clone(),
                extras,
                parent: slf.clone().unbind(),
                model,
            },
        )?;
        scene.meshes.insert(name, mesh.clone_ref(slf.py()));
        Ok(mesh)
    }

    fn __traverse__(&self, visit: PyVisit) -> Result<(), PyTraverseError> {
        visit.call(&self.parent)?;
        visit.call(&self.scene)
    }
}

#[pyclass(get_all, frozen)]
pub struct Mesh {
    pub name: String,
    pub extras: Option<Dict>,
    pub parent: Py<Node>,
    pub model: Py<Model>,
}

#[pymethods]
impl Mesh {
    fn __traverse__(&self, visit: PyVisit) -> Result<(), PyTraverseError> {
        visit.call(&self.parent)?;
        visit.call(&self.model)
    }
}
