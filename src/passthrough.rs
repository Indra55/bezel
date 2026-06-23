use anyhow::{Context, Result};
use evdev::{Device, uinput::{VirtualDevice, VirtualDeviceBuilder}};
use tracing::info;

pub fn create_virtual_device(real_device: &Device) -> Result<VirtualDevice> {
    let mut keys = evdev::AttributeSet::new();
    // Copy all keys from the real device
    if let Some(real_keys) = real_device.supported_keys() {
        for key in real_keys.iter() {
            keys.insert(key);
        }
    }

    let mut builder = VirtualDeviceBuilder::new()?.name("Bezel Virtual Trackpad");

    if let Some(real_keys) = real_device.supported_keys() {
        builder = builder.with_keys(&real_keys)?;
    }
    
    if let Some(real_abs) = real_device.supported_absolute_axes() {
        if let Ok(abs_state) = real_device.get_abs_state() {
            for axis in real_abs.iter() {
                let info = abs_state[axis.0 as usize];
                let abs_info = evdev::AbsInfo::new(
                    info.value,
                    info.minimum,
                    info.maximum,
                    info.fuzz,
                    info.flat,
                    info.resolution,
                );
                let setup = evdev::UinputAbsSetup::new(axis, abs_info);
                builder = builder.with_absolute_axis(&setup)?;
            }
        }
    }

    if let Some(real_rel) = real_device.supported_relative_axes() {
        builder = builder.with_relative_axes(&real_rel)?;
    }

    if let Some(real_switches) = real_device.supported_switches() {
        builder = builder.with_switches(&real_switches)?;
    }

    let props = real_device.properties();
    if props.iter().next().is_some() {
        builder = builder.with_properties(&props)?;
    }

    let virtual_device = builder.build().context("Failed to build virtual uinput device")?;
    
    info!("Created virtual uinput device: 'Bezel Virtual Trackpad'");
    Ok(virtual_device)
}
