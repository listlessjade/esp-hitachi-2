use std::{mem::MaybeUninit, ptr};

use esp_idf_hal::gpio::Pin;
use esp_idf_sys::{
    esp, led_color_component_format_t, led_color_component_format_t_format_layout, led_strip_clear,
    led_strip_config_t, led_strip_config_t_led_strip_extra_flags, led_strip_del,
    led_strip_handle_t, led_strip_new_rmt_device, led_strip_refresh, led_strip_rmt_config_t,
    led_strip_rmt_config_t_led_strip_rmt_extra_config, led_strip_set_pixel,
    led_strip_set_pixel_hsv, led_strip_set_pixel_rgbw,
    soc_periph_rmt_clk_src_t_RMT_CLK_SRC_DEFAULT,
};

use crate::EspResult;

#[repr(C)]
#[derive(Clone, Copy)]
pub enum LedModel {
    WS2812 = 0,
    SK6812,
    WS2811,
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct LedColorFormat {
    inner: led_color_component_format_t,
}

macro_rules! color_fmt {
    (r: $r_pos:expr, g: $g_pos:expr, b: $b_pos:expr, w: $w_pos:expr, reserved: $reserved:expr, components: $num_components:expr ) => {{
        let mut bitfield = led_color_component_format_t_format_layout::default();
        bitfield.set_r_pos($r_pos);
        bitfield.set_g_pos($g_pos);
        bitfield.set_b_pos($b_pos);
        bitfield.set_w_pos($w_pos);
        bitfield.set_reserved($reserved);
        bitfield.set_num_components($num_components);
        LedColorFormat {
            inner: led_color_component_format_t { format: bitfield },
        }
    }};
}

impl LedColorFormat {
    //#define LED_STRIP_COLOR_COMPONENT_FMT_GRB (led_color_component_format_t){.format = {.r_pos = 1, .g_pos = 0, .b_pos = 2, .w_pos = 3, .reserved = 0, .num_components = 3}}
    pub fn grb() -> Self {
        color_fmt!(r: 1, g: 0, b: 2, w: 3, reserved: 0, components: 3)
    }

    //     #define LED_STRIP_COLOR_COMPONENT_FMT_GRBW (led_color_component_format_t){.format = {.r_pos = 1, .g_pos = 0, .b_pos = 2, .w_pos = 3, .reserved = 0, .num_components = 4}}
    pub fn grbw() -> Self {
        color_fmt!(r: 1, g: 0, b: 2, w: 3, reserved: 0, components: 4)
    }

    // #define LED_STRIP_COLOR_COMPONENT_FMT_RGB (led_color_component_format_t){.format = {.r_pos = 0, .g_pos = 1, .b_pos = 2, .w_pos = 3, .reserved = 0, .num_components = 3}}
    pub fn rgb() -> Self {
        color_fmt!(r: 0, g: 1, b: 2, w: 3, reserved: 0, components: 3)
    }

    // #define LED_STRIP_COLOR_COMPONENT_FMT_RGBW (led_color_component_format_t){.format = {.r_pos = 0, .g_pos = 1, .b_pos = 2, .w_pos = 3, .reserved = 0, .num_components = 4}}
    pub fn rgbw() -> Self {
        color_fmt!(r: 0, g: 1, b: 2, w: 3, reserved: 0, components: 4)
    }
}

pub struct LedStripConfig {
    pub max_leds: u32,
    pub model: LedModel,
    pub color_format: LedColorFormat,
}

pub struct LedStrip<P: Pin> {
    #[allow(dead_code)]
    pin: P,
    handle: led_strip_handle_t,
}

impl<P: Pin> LedStrip<P> {
    pub fn new(pin: P, cfg: LedStripConfig) -> EspResult<Self> {
        let rmt_config = led_strip_rmt_config_t {
            clk_src: soc_periph_rmt_clk_src_t_RMT_CLK_SRC_DEFAULT,
            resolution_hz: 0,
            mem_block_symbols: 0,
            flags: led_strip_rmt_config_t_led_strip_rmt_extra_config::default(),
        };

        let main_config = led_strip_config_t {
            strip_gpio_num: pin.pin(),
            max_leds: cfg.max_leds,
            led_model: cfg.model as u32,
            color_component_format: cfg.color_format.inner,
            flags: led_strip_config_t_led_strip_extra_flags::default(),
        };

        let mut handle: MaybeUninit<led_strip_handle_t> = MaybeUninit::uninit();

        esp!(unsafe {
            led_strip_new_rmt_device(
                ptr::from_ref(&main_config),
                ptr::from_ref(&rmt_config),
                handle.as_mut_ptr(),
            )
        })?;

        Ok(LedStrip {
            pin,
            // cfg,
            handle: unsafe { handle.assume_init() },
        })
    }
}

impl<P: Pin> LedStrip<P> {
    pub fn set_pixel(&mut self, pixel_idx: u32, color: (u32, u32, u32)) -> EspResult<()> {
        let (red, green, blue) = color;
        esp!(unsafe { led_strip_set_pixel(self.handle, pixel_idx, red, green, blue) })
    }

    pub fn set_pixel_rgbw(&mut self, pixel_idx: u32, color: (u32, u32, u32, u32)) -> EspResult<()> {
        let (red, green, blue, white) = color;
        esp!(unsafe { led_strip_set_pixel_rgbw(self.handle, pixel_idx, red, green, blue, white) })
    }

    pub fn set_pixel_hsv(&mut self, pixel_idx: u32, color: (u16, u8, u8)) -> EspResult<()> {
        let (h, s, v) = color;
        esp!(unsafe { led_strip_set_pixel_hsv(self.handle, pixel_idx, h, s, v) })
    }

    /// turns off all leds
    pub fn clear(&mut self) -> EspResult<()> {
        esp!(unsafe { led_strip_clear(self.handle) })
    }

    /// writes colors to LEDs
    pub fn write(&mut self) -> EspResult<()> {
        esp!(unsafe { led_strip_refresh(self.handle) })
    }
}

impl<P: Pin> Drop for LedStrip<P> {
    fn drop(&mut self) {
        // log this ? somehow
        let _ = esp!(unsafe { led_strip_del(self.handle) });
    }
}
