# esp-hitachi firmware... 2!

This work is licensed under a
[Creative Commons Attribution-NonCommercial-ShareAlike 4.0 International License][cc-by-nc-sa].

[![CC BY-NC-SA 4.0][cc-by-nc-sa-image]][cc-by-nc-sa]

[cc-by-nc-sa]: http://creativecommons.org/licenses/by-nc-sa/4.0/
[cc-by-nc-sa-image]: https://licensebuttons.net/l/by-nc-sa/4.0/88x31.png
[cc-by-nc-sa-shield]: https://img.shields.io/badge/License-CC%20BY--NC--SA%204.0-lightgrey.svg



### OTA procedure

1. build the compressed firmware image with `idf.py gen_compressed_ota`
2. upload it with `curl --data-binary "@build/custom_ota_binaries/esp-cmake.bin.xz.packed" --header "Content-Type: application/octet-stream" http://ip:port/ota/upload`

### Setting Up Wifi

1. connect to UART via BLE (use a Nordic BLE UART compatible client - e.g Bluefruit Connect)
2. invoke the following incantations:
```
wifi -f ssid set "SSID"
wifi -f password set "PASSWORD"
```
3. restart the wand via BLE 
```
restart
```
or just unplug it 