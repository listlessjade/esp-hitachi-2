#include <stdbool.h>

// #include "nimble/nimble_port.h"
// #include "nimble/nimble_port_freertos.h"
// #include "host/ble_hs.h"
// #include "host/util/util.h"
// #include "host/ble_uuid.h"
// #include "services/gap/ble_svc_gap.h"
// #include "services/gatt/ble_svc_gatt.h"
// #include "esp_nimble_hci.h"
// #include "esp_gap_ble_api.h"
// #include "esp_gatt_defs.h"
// #include "esp_gatt_common_api.h"
// #include "esp_gatts_api.h"
#ifdef ESP_IDF_COMP_ESPRESSIF__BUTTON_ENABLED
#include "button_interface.h"
#include "button_adc.h"
#include "button_gpio.h"
#include "button_matrix.h"
#include "button_types.h"
#include "iot_button.h"
#endif

#ifdef ESP_IDF_COMP_ESPRESSIF__LED_STRIP_ENABLED
#include "led_strip.h"
#include "led_strip_interface.h"
#include "led_strip_rmt.h"
#include "led_strip_spi.h"
#include "led_strip_types.h"
#endif

#ifdef ESP_IDF_COMP_ESPRESSIF__BOOTLOADER_SUPORT_PLUS_ENABLED
#include "bootloader_custom_ota.h"
#endif

#ifdef ESP_IDF_COMP_ESPRESSIF__NTC_DRIVER_ENABLED
#include "ntc_driver.h"
#endif

#include "esp_littlefs.h"
