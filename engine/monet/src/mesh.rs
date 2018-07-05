pub use descartes::{N, P3, P2, V3, V4, M4, Iso3, Persp3, Into2d, Into3d, WithUniqueOrthogonal,
Area, Band};

use glium::{self, index};
use glium::backend::glutin::Display;

use compact::CVec;

#[derive(Copy, Clone, Debug)]
pub struct Vertex {
    pub position: [f32; 3],
}

implement_vertex!(Vertex, position);

#[derive(Copy, Clone)]
pub struct Instance {
    pub instance_position: [f32; 3],
    pub instance_direction: [f32; 2],
    pub instance_color: [f32; 3],
}

implement_vertex!(
    Instance,
    instance_position,
    instance_direction,
    instance_color
);

impl Instance {
    pub fn with_color(color: [f32; 3]) -> Instance {
        Instance {
            instance_position: [0.0, 0.0, 0.0],
            instance_direction: [1.0, 0.0],
            instance_color: color,
        }
    }
}

#[derive(Compact, Debug)]
pub struct Mesh {
    pub vertices: CVec<Vertex>,
    pub indices: CVec<u16>,
}

impl Mesh {
    pub fn new(vertices: Vec<Vertex>, indices: Vec<u16>) -> Mesh {
        Mesh {
            vertices: vertices.into(),
            indices: indices.into(),
        }
    }

    pub fn empty() -> Mesh {
        Mesh {
            vertices: CVec::new(),
            indices: CVec::new(),
        }
    }
}

impl Clone for Mesh {
    fn clone(&self) -> Mesh {
        Mesh {
            vertices: self.vertices.to_vec().into(),
            indices: self.indices.to_vec().into(),
        }
    }
}

impl ::std::ops::Add for Mesh {
    type Output = Mesh;

    fn add(mut self, rhs: Mesh) -> Mesh {
        let self_n_vertices = self.vertices.len();
        self.vertices.extend_from_copy_slice(&rhs.vertices);
        self.indices
            .extend(rhs.indices.iter().map(|i| *i + self_n_vertices as u16));
        self
    }
}

impl ::std::ops::AddAssign for Mesh {
    fn add_assign(&mut self, rhs: Mesh) {
        let self_n_vertices = self.vertices.len();
        for vertex in rhs.vertices.iter().cloned() {
            self.vertices.push(vertex);
        }
        for index in rhs.indices.iter() {
            self.indices.push(index + self_n_vertices as u16)
        }
    }
}

impl ::std::iter::Sum for Mesh {
    fn sum<I: Iterator<Item = Mesh>>(iter: I) -> Mesh {
        let mut summed_mesh = Mesh {
            vertices: CVec::new(),
            indices: CVec::new(),
        };
        for mesh in iter {
            summed_mesh += mesh;
        }
        summed_mesh
    }
}

impl<'a> ::std::ops::AddAssign<&'a Mesh> for Mesh {
    fn add_assign(&mut self, rhs: &'a Mesh) {
        let self_n_vertices = self.vertices.len();
        for vertex in rhs.vertices.iter().cloned() {
            self.vertices.push(vertex);
        }
        for index in rhs.indices.iter() {
            self.indices.push(index + self_n_vertices as u16)
        }
    }
}

impl<'a> ::std::iter::Sum<&'a Mesh> for Mesh {
    fn sum<I: Iterator<Item = &'a Mesh>>(iter: I) -> Mesh {
        let mut summed_mesh = Mesh {
            vertices: CVec::new(),
            indices: CVec::new(),
        };
        for mesh in iter {
            summed_mesh += mesh;
        }
        summed_mesh
    }
}

use itertools::{Itertools, Position};
use lyon_tessellation::{FillTessellator, FillOptions, FillVertex, GeometryBuilder};
use lyon_tessellation::geometry_builder::{VertexId, Count};
use lyon_tessellation::path::iterator::PathIter;
use lyon_tessellation::path::PathEvent;
use lyon_tessellation::math::point;

impl GeometryBuilder<FillVertex> for Mesh {
    fn begin_geometry(&mut self) {}
    fn end_geometry(&mut self) -> Count {
        Count {
            vertices: self.vertices.len() as u32,
            indices: self.indices.len() as u32,
        }
    }
    fn abort_geometry(&mut self) {}
    fn add_vertex(&mut self, input: FillVertex) -> VertexId {
        let id = self.vertices.len();
        self.vertices.push(Vertex {
            position: [input.position.x, input.position.y, 0.0],
        });
        VertexId(id as u16)
    }
    fn add_triangle(&mut self, a: VertexId, b: VertexId, c: VertexId) {
        self.indices.push(a.0);
        self.indices.push(b.0);
        self.indices.push(c.0);
    }
}

impl Mesh {
    pub fn from_area(area: &Area) -> Mesh {
        let path_iterator = PathIter::new(area.primitives.iter().flat_map(|primitive| {
            primitive
                .boundary
                .path()
                .segments()
                .with_position()
                .flat_map(|segment_with_position| {
                    let initial_move = match segment_with_position {
                        Position::First(segment) | Position::Only(segment) => Some(
                            PathEvent::MoveTo(point(segment.start().x, segment.start().y)),
                        ),
                        _ => None,
                    };

                    let segment = segment_with_position.into_inner();

                    initial_move
                        .into_iter()
                        .chain(Some(PathEvent::LineTo(point(
                            segment.end().x,
                            segment.end().y,
                        ))))
                        .collect::<Vec<_>>()
                })
        }));

        let mut tesselator = FillTessellator::new();
        let mut output = Mesh::empty();

        tesselator
            .tessellate_path(path_iterator, &FillOptions::default(), &mut output)
            .unwrap();

        output
    }

    pub fn from_band(band: &Band, z: N) -> Mesh {
        fn to_vertex(point: P2, z: N) -> Vertex {
            Vertex {
                position: [point.x, point.y, z],
            }
        }

        let left = band
            .path
            .shift_orthogonally(-band.width_left)
            .unwrap_or_else(|| band.path.clone());
        let right = band
            .path
            .shift_orthogonally(band.width_right)
            .unwrap_or_else(|| band.path.clone());

        let vertices = left
            .points
            .iter()
            .chain(right.points.iter())
            .map(|&p| to_vertex(p, z))
            .collect::<Vec<_>>();

        let left_len = left.points.len();

        let indices = (0..(left_len - 1))
            .flat_map(|left_i| {
                let left_i = left_i as u16;
                let right_i = left_i + left_len as u16;

                vec![
                    left_i,
                    right_i.min(vertices.len() as u16 - 1),
                    left_i + 1,
                    left_i + 1,
                    right_i.min(vertices.len() as u16 - 1),
                    (right_i + 1).min(vertices.len() as u16 - 1),
                ]
            })
            .collect();

        Mesh::new(vertices, indices)
    }
}

pub struct Batch {
    pub vertices: glium::VertexBuffer<Vertex>,
    pub indices: glium::IndexBuffer<u16>,
    pub instances: Vec<Instance>,
    pub clear_every_frame: bool,
    pub full_frame_instance_end: Option<usize>,
    pub is_decal: bool,
    pub frame: usize,
}

use std::net::{TcpStream};
use tungstenite::{WebSocket, Message};
use byteorder::{LittleEndian, WriteBytesExt};

impl Batch {
    pub fn new(
        id: u32,
        prototype: &Mesh,
        window: &Display,
        websocket: &mut WebSocket<TcpStream>,
    ) -> Batch {
        transfer_batch(id, prototype, websocket);

        Batch {
            vertices: glium::VertexBuffer::new(window, &prototype.vertices).unwrap(),
            indices: glium::IndexBuffer::new(
                window,
                index::PrimitiveType::TrianglesList,
                &prototype.indices,
            ).unwrap(),
            instances: Vec::new(),
            full_frame_instance_end: None,
            clear_every_frame: true,
            is_decal: false,
            frame: 0,
        }
    }

    pub fn new_individual(
        id: u32,
        mesh: &Mesh,
        instance: Instance,
        is_decal: bool,
        window: &Display,
        websocket: &mut WebSocket<TcpStream>,
    ) -> Batch {
        transfer_batch(id, mesh, websocket);

        Batch {
            vertices: glium::VertexBuffer::new(window, &mesh.vertices).unwrap(),
            indices: glium::IndexBuffer::new(
                window,
                index::PrimitiveType::TrianglesList,
                &mesh.indices,
            ).unwrap(),
            instances: vec![instance],
            clear_every_frame: false,
            full_frame_instance_end: None,
            is_decal,
            frame: 0,
        }
    }
}

fn transfer_batch(id: u32, mesh: &Mesh, websocket: &mut WebSocket<TcpStream>) {
    let Mesh {
        ref vertices,
        ref indices,
    } = mesh;
    let mut websocket_message = Vec::<u8>::new();

    if vertices.is_empty() || indices.is_empty() {
        return;
    }

    // batch creation
    websocket_message.write_u32::<LittleEndian>(13).unwrap();

    websocket_message.write_u32::<LittleEndian>(id).unwrap();

    websocket_message
        .write_u32::<LittleEndian>(vertices.len() as u32)
        .unwrap();
    let vertices_pos = websocket_message.len();
    websocket_message.resize(
        vertices_pos + vertices.len() * ::std::mem::size_of::<Vertex>(),
        0,
    );
    unsafe {
        vertices.as_ptr().copy_to(
            &mut websocket_message[vertices_pos] as *mut u8 as *mut Vertex,
            vertices.len(),
        )
    }

    websocket_message
        .write_u32::<LittleEndian>(indices.len() as u32)
        .unwrap();
    let indices_pos = websocket_message.len();
    websocket_message.resize(
        indices_pos + indices.len() * ::std::mem::size_of::<u16>(),
        0,
    );
    unsafe {
        indices.as_ptr().copy_to(
            &mut websocket_message[indices_pos] as *mut u8 as *mut u16,
            indices.len(),
        )
    }

    websocket
        .write_message(Message::binary(websocket_message))
        .unwrap();
}
