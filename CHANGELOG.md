## 1.1.4 (unreleased)

## 1.1.3 (10.06.2024)
- Update rustyline to v13.0
- Replace unmaintained atty crate with standard library implementation

## 1.1.2 (05.03.2024)
- Add history element even if parsing fails
- Disable SSL when connecting to localhost
- Fix crash when only NaNs are returned from raw command
- Update various crates

## 1.1.1 (06.12.2022)
- Add typed input to history instead of generated json
- Add shortcut for hidden values with `--hidden`
- Add shortcuts `sn` for `serial_number` and `bt` for `board_temp`
- Add shortcut for `sync` command

## 1.1.0 (04.12.2022)
- Add shortcuts for `channel_data` and `chanel_attributes`
- Update libraries
- Add support for ssl encryption (enabled by default)
- Allow to use `--serial` to generate link local ipv6 adress for given serial

## 1.0.0 (09.06.2021)
- Add support for `ks_sync` and `ks` command
- Add second level for quick commands, if parameter two is not json, it will be sent as name

## 0.1.0 (05.05.2021)
- Initial release