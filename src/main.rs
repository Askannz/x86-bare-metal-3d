#![no_std]
#![no_main]

use core::{cmp::Ordering, panic::PanicInfo};
use micromath::F32Ext;

const H: usize = 25;
const W: usize = 80;
const COLORS: [u8; 7] = [0x0, 0x9, 0xd, 0xb, 0xa, 0xc, 0xe];
const ZOOM: f32 = 1.1;
const PI: f32 = 3.14159265359;
const NB_QUADS: usize = 6;
const VIEW_PITCH_MAX: f32 = PI / 4.0;

const SUPERSAMPLING: usize = 2;
const BUF_H: usize = SUPERSAMPLING * H;
const BUF_W: usize = SUPERSAMPLING * W;

const MIN_COLOR: u8 = 0x0;
const MAX_COLOR: u8 = 0xe;
const COLORS_N: usize = (MAX_COLOR - MIN_COLOR + 1) as usize;

const PX_RATIO: f32 = 16.0 / 9.0;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    
    let mut buffer = [0u8; BUF_H * BUF_W];

    let base_quad = [
        Point { x: -1.0, y: -1.0, z: -1.0 },
        Point { x: 1.0, y: -1.0, z: -1.0 },
        Point { x: 1.0, y: 1.0, z: -1.0 },
        Point { x: -1.0, y: 1.0, z: -1.0 }
    ];

    let zero_point = Point {x: 0.0, y: 0.0, z: 0.0};
    let zero_quad = [zero_point; 4];
    let mut geometry = [zero_quad; NB_QUADS];
    for i in 0..4 {
        let i_f = i as f32;
        geometry[i] = rotate(&base_quad, Axis::Y, i_f * PI / 2.0);
    }
    geometry[4] = rotate(&base_quad, Axis::X, - PI / 2.0);
    geometry[5] = rotate(&base_quad, Axis::X, PI / 2.0);

    let mut still_counter = 0;
    let mut view_yaw = 0.0;
    let mut pitch_v: f32 = 0.0;

    loop {

        let mut geometry = geometry.clone();

        geometry.iter_mut().for_each(|quad| {
            *quad = rotate(quad, Axis::Y, view_yaw);
        });

        let pitch = VIEW_PITCH_MAX * pitch_v.sin();
    
        geometry.iter_mut().for_each(|quad| {
            *quad = rotate(quad, Axis::X, pitch);
        });

        rasterize(&mut buffer, &geometry);
        draw(&buffer);

        if still_counter == 10 {
            view_yaw += 0.2;
            pitch_v += 0.1;
            still_counter = 0;
        }

        still_counter += 1;
    }
}

fn draw(buffer: &[u8]) {

    let vga = 0xb8000 as *mut u8;

    for x_vga in 0..W {
        for y_vga in 0..H {

            let i_vga = y_vga * W + x_vga;
            let x_ss = x_vga * SUPERSAMPLING;
            let y_ss = y_vga * SUPERSAMPLING;

            let (codepoint_byte, color_byte) = get_VGA_bytes(buffer, x_ss, y_ss);

            unsafe {
                *vga.offset(i_vga as isize * 2) = codepoint_byte;
                *vga.offset(i_vga as isize * 2 + 1) = color_byte;
            }

        }
    }
}

fn get_VGA_bytes(buffer: &[u8], x: usize, y: usize) -> (u8, u8) {

    let mut counter = [0usize; COLORS_N];

    for dx in 0..SUPERSAMPLING {
        for dy in 0..SUPERSAMPLING {
            let color = buffer[(y + dy) * BUF_W + x + dx];
            counter[(color - MIN_COLOR) as usize] += 1;
        }
    }

    let (i1, c1) = counter.iter().enumerate()
        .max_by_key(|(_i, c)| *c)
        .unwrap();

    let color_1 = (i1 as u8) + MIN_COLOR;

    let opt = counter.iter().enumerate().find(|(i, c)| **c > 0 && *i != i1);

    if SUPERSAMPLING != 2 {
        unimplemented!()
    }

    let (codepoint_byte, color_byte) = match opt {

        None => (0xdb, (i1 as u8) + MIN_COLOR),
        Some((i2, c2)) => {

            let c1 = *c1 as i32;
            let c2 = *c2 as i32;
            let color_2 = (i2 as u8) + MIN_COLOR;

            if c1 == 3 && c2 == 1 { (0xb2, (color_2 << 4) + color_1) }
            else if c1 == 2 && c2 <= 2 { (0xb1, (color_2 << 4) + color_1) }
            else { (0x40, 0xf) }
        }
    };

    return (codepoint_byte, color_byte)
}

fn rotate(poly: &Quad, axis: Axis, angle: f32) -> Quad {

    let mat = match axis {
        Axis::X => [
            1.0, 0.0, 0.0,
            0.0, angle.cos(), -angle.sin(),
            0.0, angle.sin(), angle.cos()
        ],

        Axis::Y => [
            angle.cos(), 0.0, angle.sin(),
            0.0, 1.0, 0.0,
            -angle.sin(), 0.0, angle.cos()
        ],

        Axis::Z => [
            angle.cos(), -angle.sin(), 0.0,
            angle.sin(), angle.cos(), 0.0,
            0.0, 0.0, 1.0
        ]
    };

    let mut new_poly: Quad = [Point {x: 0.0, y: 0.0, z: 0.0}; 4];

    for (i, p) in poly.iter().enumerate() {
        let new_p = matmul(&mat, p);
        new_poly[i] = new_p;
    }

    new_poly
}

fn rasterize(buffer: &mut [u8], geometry: &[Quad; NB_QUADS]) {

    buffer.fill(0);

    for (i, poly) in geometry.iter().enumerate() {
        let color = COLORS[i % (COLORS.len() - 1) + 1];
        rasterize_poly(buffer, poly, color);
    }

}

fn rasterize_poly(buffer: &mut [u8], poly: &Quad, color: u8) {

    let cmp_f = |a: &f32, b: &f32| { a.partial_cmp(b).unwrap_or(Ordering::Equal) };
    let min_x = poly.iter().map(|p| p.x).min_by(cmp_f).unwrap();
    let max_x = poly.iter().map(|p| p.x).max_by(cmp_f).unwrap();
    let min_y = poly.iter().map(|p| p.y).min_by(cmp_f).unwrap();
    let max_y = poly.iter().map(|p| p.y).max_by(cmp_f).unwrap();

    for x_px in 0..BUF_W {
        for y_px in 0..BUF_H {

            let p = {
   
                let x_px = x_px as f32;
                let y_px = y_px as f32;
                let w = BUF_W as f32;
                let h = BUF_H as f32;

                let rx = 2.0 * (x_px - (w - h) / 2.0) / (h - 1.0);
                let ry = 2.0 * y_px / (h - 1.0);

                Point {
                    x: (rx - 1.0) / ZOOM,
                    y: PX_RATIO * (ry - 1.0) / ZOOM,
                    z: 0.0
                }
            };

            if p.x < min_x || p.x > max_x || p.y < min_y || p.y > max_y {
                continue;
            }

            if test_in_poly(&poly, &p) {
                buffer[y_px * BUF_W + x_px] = color;
            }
        }
    }

}

fn test_in_poly(poly: &Quad, p: &Point) -> bool {

    let n = poly.len();

    for i1 in 0..n {

        let p1 = poly[i1];
        let p2 = poly[(i1 + 1) % n];

        let d = (p2.x - p1.x) * (p.y - p1.y) - (p2.y - p1.y) * (p.x - p1.x);

        if d < 0.0 { return false; }
    }

    return true;
}

fn matmul(m: &Matrix, vec: &Vector) -> Vector {

    let v = [vec.x, vec.y, vec.z];

    Vector {
        x: m[0] * v[0] + m[1] * v[1] + m[2] * v[2],
        y: m[3] * v[0] + m[4] * v[1] + m[5] * v[2],
        z: m[6] * v[0] + m[7] * v[1] + m[8] * v[2]
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) ->  ! {
    loop {}
}

#[derive(Debug, Clone, Copy)]
struct Vector { x: f32, y: f32, z: f32 }
type Point = Vector;
type Quad = [Point; 4];
type Matrix = [f32; 9];

#[derive(Debug, Clone, Copy)]
enum Axis { X, Y, Z }
