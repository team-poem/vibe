use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

pub fn request_microphone() -> Result<String, String> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| "no default input device".to_string())?;
    let name = device.name().unwrap_or_else(|_| "unknown".to_string());
    let config = device
        .default_input_config()
        .map_err(|e| format!("default input config: {e}"))?;
    let stream = device
        .build_input_stream(
            &config.into(),
            |_data: &[f32], _: &cpal::InputCallbackInfo| {},
            |err| eprintln!("[mic] stream error: {err}"),
            None,
        )
        .map_err(|e| format!("build input stream: {e}"))?;
    stream.play().map_err(|e| format!("play stream: {e}"))?;
    drop(stream);
    Ok(name)
}
