# Reference docs

Datasheets, schematics, and user guides for the hardware this workspace targets, kept locally
for offline reference while writing/verifying BSPs. PDFs and spreadsheets are large and not
tracked in git (see `.gitignore`); pinout images and the saved HTML pages are small enough to
commit.

- [`chip/`](chip) -- ESP32-C6 chip datasheet and technical reference manual (applies to every
  board below; from [espressif.com](https://www.espressif.com/en/support/documents/technical-documents)).
- [`devkitc-1/`](devkitc-1) -- Espressif [ESP32-C6-DevKitC-1](https://docs.espressif.com/projects/esp-dev-kits/en/latest/esp32c6/esp32-c6-devkitc-1/user_guide.html)
  (ESP32-C6-WROOM-1 module, 8 MB flash): user guide, pin layout, schematics, PCB layout,
  dimensions, and the WROOM-1/WROOM-1U module datasheet.
- [`devkitm-1/`](devkitm-1) -- Espressif [ESP32-C6-DevKitM-1](https://docs.espressif.com/projects/esp-dev-kits/en/latest/esp32c6/esp32-c6-devkitm-1/user_guide.html)
  (ESP32-C6-MINI-1 module, 4 MB flash): same set of docs, plus the MINI-1/MINI-1U module
  datasheet.
- [`xiao-esp32c6/`](xiao-esp32c6) -- Seeed Studio [XIAO ESP32C6](https://wiki.seeedstudio.com/xiao_esp32c6_getting_started/):
  getting-started wiki page, front/back pinout diagrams, schematic, and pinout spreadsheet.

Source of truth for revisions: `devkitc-1`/`devkitm-1` schematics are Espressif's latest as of
2026-10-20 (DevKitC-1 schematic v1.4, board rev v1.2); the XIAO schematic is Seeed's v1.0.
Re-download from the linked pages if a newer hardware revision ships.
