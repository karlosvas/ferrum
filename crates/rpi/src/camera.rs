use anyhow::{Context, Result};
use clap::Parser;
use image::{GenericImageView, ImageReader};
use serde::{Deserialize, Serialize};
use std::process::Command;

const IMAGE_WIDTH: u32 = 2304;
const IMAGE_HEIGHT: u32 = 1296;
const DEFAULT_BORDER_PERCENT: u32 = 10;

#[derive(Debug)]
#[command(
    name = "light-direction-local",
    version,
    about = "Light direction detector - runs locally on Pi"
)]
struct Args {
    /// Output format
    #[arg(long, value_enum, default_value = "brief")]
    format: OutputFormat,

    /// Border exclusion percentage
    #[arg(long, default_value_t = DEFAULT_BORDER_PERCENT)]
    border: u32,

    /// Use existing image file instead of capturing
    #[arg(long)]
    image: Option<String>,
}

#[derive(Debug, Clone, ValueEnum)]
enum OutputFormat {
    Json,
    Angle,
    Direction,
    Brief,
}

#[derive(Debug, Serialize, Deserialize)]
struct LightResult {
    brightest_pixel: PixelInfo,
    image_center: PixelInfo,
    vector: Vector,
    angle_degrees: f32,
    direction: String,
    search_region: SearchRegion,
}

#[derive(Debug, Serialize, Deserialize)]
struct PixelInfo {
    x: u32,
    y: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    brightness: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Vector {
    dx: i32,
    dy: i32,
}

#[derive(Debug, Serialize, Deserialize)]
struct SearchRegion {
    x: [u32; 2],
    y: [u32; 2],
    border_percent: u32,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let local_image = if let Some(path) = args.image {
        path
    } else {
        let path = "/tmp/light_detect.jpg".to_string();
        capture_locally(&path)?;
        path
    };

    let result = analyze_image(&local_image, args.border)?;

    match args.format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&result)?),
        OutputFormat::Angle => println!("{:.1}", result.angle_degrees),
        OutputFormat::Direction => println!("{}", result.direction),
        OutputFormat::Brief => {
            println!("Light Direction: {}", result.direction);
            println!("Angle: {:.1}°", result.angle_degrees);
            println!(
                "Brightest pixel: ({}, {})",
                result.brightest_pixel.x, result.brightest_pixel.y
            );
            println!();
            println!("Camera pointing UP — positive angle = RIGHT, negative = LEFT");
        }
    }

    Ok(())
}

fn capture_locally(output_path: &str) -> Result<()> {
    eprintln!("Capturing image locally...");
    let status = Command::new("rpicam-jpeg")
        .args([
            "-o",
            output_path,
            "--width",
            &IMAGE_WIDTH.to_string(),
            "--height",
            &IMAGE_HEIGHT.to_string(),
            "--timeout",
            "2000",
            "--nopreview",
        ])
        .status()
        .context("Failed to run rpicam-jpeg")?;

    if !status.success() {
        anyhow::bail!("rpicam-jpeg failed with status: {}", status);
    }
    Ok(())
}

fn analyze_image(image_path: &str, border_percent: u32) -> Result<LightResult> {
    eprintln!("Analyzing image...");

    let img = ImageReader::open(image_path)?
        .decode()
        .context("Failed to decode image")?;

    let (w, h) = img.dimensions();

    // Convert to grayscale for brightness analysis
    let gray = img.to_luma8();

    // Border exclusion
    let bx = (w * border_percent / 100).max(1);
    let by = (h * border_percent / 100).max(1);
    let sx = bx;
    let ex = w - bx;
    let sy = by;
    let ey = h - by;

    let cx = w / 2;
    let cy = h / 2;

    // Find brightest pixel
    let mut max_bright = 0.0f32;
    let mut max_x = 0;
    let mut max_y = 0;

    for y in sy..ey {
        for x in sx..ex {
            let pixel = gray.get_pixel(x, y);
            let brightness = pixel[0] as f32;
            if brightness > max_bright {
                max_bright = brightness;
                max_x = x;
                max_y = y;
            }
        }
    }

    // Calculate angle (camera pointing UP)
    // Image coordinates: x right, y down
    // UP = -y direction
    let dx = max_x as i32 - cx as i32;
    let dy = max_y as i32 - cy as i32;

    // atan2(dx, -dy) for angle where 0° = up, + = right, - = left
    let angle_rad = (dx as f32).atan2(-(dy as f32));
    let angle_deg = angle_rad.to_degrees();

    let direction = if angle_deg > 15.0 {
        "RIGHT"
    } else if angle_deg < -15.0 {
        "LEFT"
    } else {
        "CENTER"
    };

    Ok(LightResult {
        brightest_pixel: PixelInfo {
            x: max_x,
            y: max_y,
            brightness: Some(max_bright),
        },
        image_center: PixelInfo {
            x: cx,
            y: cy,
            brightness: None,
        },
        vector: Vector { dx, dy },
        angle_degrees: (angle_deg * 10.0).round() / 10.0,
        direction: direction.to_string(),
        search_region: SearchRegion {
            x: [sx, ex],
            y: [sy, ey],
            border_percent,
        },
    })
}
