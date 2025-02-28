use esp_idf_hal::gpio::Pin;
/*

20 steps = 4 * 5

0-3 = rgb(0, 17, 199)

0 = [1, 0, 0, 0]
1 = [1, 1, 0, 0]
2 = [1, 1, 1, 0]
3 = [1, 1, 1, 1]


4-8 (4, 5, 6, 7) = rgb(0, 176, 199)


8-12 (8, 9, 10, 11)


4 = [1, 0, 0, 0]

9-12 = rgb(0, 199, 36)
12-16 = rgb(199, 116, 0)
16-20 = rgb(199, 0, 20)

start at ~50/100 on duty cycle

*/

// use smart_leds::RGB;
// use ws2812_esp32_rmt_driver::Ws2812Esp32Rmt;

use crate::idf_libs::led_strip::LedColorFormat;
use crate::idf_libs::led_strip::LedModel::WS2812;
use crate::idf_libs::led_strip::LedStrip;
use crate::idf_libs::led_strip::LedStripConfig;
use crate::EspResult;

pub struct Lights<P: Pin> {
    pub led: LedStrip<P>,
}

impl<P: Pin> Lights<P> {
    pub fn new(pin: P) -> EspResult<Self> {
        let strip = LedStrip::new(
            pin,
            LedStripConfig {
                max_leds: 4,
                model: WS2812,
                color_format: LedColorFormat::grb(),
            },
        )?;

        Ok(Self { led: strip })
    }

    pub fn set_all(&mut self, pixels: [(u32, u32, u32); 4]) -> EspResult<()> {
        for (idx, color) in pixels.into_iter().enumerate() {
            self.led.set_pixel(idx as u32, color)?;
        }

        self.led.write()?;

        Ok(())
    }

    pub fn show_speed(&mut self, pwr: u32) -> EspResult<()> {
        if pwr == 0 {
            return self.set_all([(0, 0, 0), (0, 0, 0), (0, 0, 0), (100, 100, 100)]);
        }

        let led_qnt = if pwr < 20 { pwr % 4 } else { 4 };

        let lit_color = match pwr {
            0..4 => (0, 17, 199),
            4..8 => (0, 176, 199),
            8..12 => (0, 199, 36),
            12..16 => (199, 116, 0),
            16.. => (199, 0, 20),
        };

        let black = (0, 0, 0);

        // // this could be a formula or a for but i'm so sleepy right now istg
        let lit_leds = match led_qnt {
            0 => [black, black, black, lit_color],
            1 => [black, black, lit_color, lit_color],
            2 => [black, lit_color, lit_color, lit_color],
            _ => [lit_color, lit_color, lit_color, lit_color],
        };

        // println!("{pwr} => {led_qnt} => {lit_leds:?}");

        self.set_all(lit_leds)

        // self.led
        //     .write(
        //         std::iter::repeat(color)
        //             .take(led_qnt as usize)
        //             .chain(std::iter::repeat(RGB::new(0, 0, 0)).take(4 - led_qnt as usize))
        //             .rev(),
        //     )
        //     .unwrap();
    }
}
