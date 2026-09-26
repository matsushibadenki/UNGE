use std::collections::BTreeSet;
use unge_core::*;
use unge_render::*;
fn document() -> Document {
    let mut editor = Editor::new(Document::default(), 0).unwrap();
    editor
        .execute(Command::AddNode {
            node: Node {
                id: Id::new_v4(),
                type_id: "test".into(),
                inputs: vec![],
                outputs: vec![],
                properties: Properties::new(),
            },
            rect: Rect {
                x: 20.,
                y: 20.,
                width: 180.,
                height: 90.,
            },
        })
        .unwrap();
    editor.document().clone()
}
#[test]
fn culls_nodes_and_reduces_ports_at_low_zoom() {
    let index = SceneIndex::new(&document());
    let view = Viewport {
        origin: [0., 0.],
        zoom: 1.,
        size: [256., 256.],
    };
    assert_eq!(
        index.scene(view, &BTreeSet::new()).unwrap().visible_nodes,
        1
    );
    assert_eq!(
        index
            .scene(
                Viewport {
                    origin: [1000., 1000.],
                    ..view
                },
                &BTreeSet::new()
            )
            .unwrap()
            .visible_nodes,
        0
    );
    assert!(
        index
            .scene(Viewport { zoom: 0., ..view }, &BTreeSet::new())
            .is_err()
    );
}
#[test]
fn edges_crossing_viewport_survive_endpoint_culling() {
    let port = Port {
        name: "p".into(),
        data_type: DataType::Float,
        cardinality: Cardinality::Single,
        required: true,
    };
    let a = Node {
        id: Id::new_v4(),
        type_id: "test".into(),
        inputs: vec![port.clone()],
        outputs: vec![port],
        properties: Properties::new(),
    };
    let b = Node {
        id: Id::new_v4(),
        ..a.clone()
    };
    let (aid, bid) = (a.id, b.id);
    let mut editor = Editor::new(Document::default(), 0).unwrap();
    editor
        .execute(Command::Batch {
            commands: vec![
                Command::AddNode {
                    node: a,
                    rect: Rect {
                        x: -400.,
                        y: 10.,
                        width: 180.,
                        height: 90.,
                    },
                },
                Command::AddNode {
                    node: b,
                    rect: Rect {
                        x: 400.,
                        y: 10.,
                        width: 180.,
                        height: 90.,
                    },
                },
                Command::Connect {
                    edge: Edge {
                        id: Id::new_v4(),
                        from: Endpoint {
                            node: aid,
                            port: "p".into(),
                        },
                        to: Endpoint {
                            node: bid,
                            port: "p".into(),
                        },
                    },
                },
            ],
        })
        .unwrap();
    let scene = SceneIndex::new(editor.document())
        .scene(
            Viewport {
                origin: [0., 0.],
                zoom: 1.,
                size: [200., 200.],
            },
            &BTreeSet::new(),
        )
        .unwrap();
    assert_eq!(scene.visible_nodes, 0);
    assert_eq!(scene.visible_edges, 1);
}
#[test]
#[ignore = "requires a working GPU adapter; run explicitly on a desktop"]
fn gpu_renders_pixels_without_validation_errors() {
    pollster::block_on(async {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .expect("GPU adapter required");
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .unwrap();
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 256,
                height: 256,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let mut renderer = GpuRenderer::new(&device, wgpu::TextureFormat::Rgba8Unorm);
        let viewport = Viewport {
            origin: [0., 0.],
            zoom: 1.,
            size: [256., 256.],
        };
        let scene = SceneIndex::new(&document())
            .scene(viewport, &BTreeSet::new())
            .unwrap();
        renderer.prepare(&device, &queue, &scene, viewport).unwrap();
        let mut encoder = device.create_command_encoder(&Default::default());
        renderer.render(&mut encoder, &texture.create_view(&Default::default()));
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 256 * 256 * 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(1024),
                    rows_per_image: Some(256),
                },
            },
            wgpu::Extent3d {
                width: 256,
                height: 256,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                tx.send(result).unwrap();
            });
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        rx.recv().unwrap().unwrap();
        let pixels = buffer.slice(..).get_mapped_range();
        let center = &pixels[(60 * 256 + 60) * 4..(60 * 256 + 60) * 4 + 4];
        assert!(
            center[0] > 18 && center[0] < 30,
            "node body pixel: {center:?}"
        );
        assert_eq!(center[3], 255);
        let background = &pixels[(200 * 256 + 200) * 4..(200 * 256 + 200) * 4 + 4];
        assert!(background[0] < 18);
        drop(pixels);
        buffer.unmap();
        assert!(device.pop_error_scope().await.is_none());
    });
}
