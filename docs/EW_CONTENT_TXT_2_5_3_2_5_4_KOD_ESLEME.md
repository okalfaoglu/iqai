# `content.txt` §2.5.3–2.5.4 ↔ Kod eşlemesi

## Config

- `smart_money.elliott_thesis_te_y_rules` (`config.json` → `Config::elliott_thesis_te_y_rules`). Eski JSON anahtarı `elliott_thesis_teY_rules` hâlâ kabul edilir (`SmartMoneyConfig` serde `alias`).
  - **`false` (varsayılan):** İtki `formation_valid` tez W3/W4 ek koşulları **hariç**; zigzag `validate_zigzag_abc` EWM bandı (B 38.2–85.4%).
  - **`true`:** İtki için `validate_impulse_with_w5(..., apply_thesis_te_y: true)` → W3 ucu W1 bitişinin ötesinde, `|W4| ≤ |W3|`; zigzag için B üst sınır **%61.8** ve **C genliği ≥ B**.

## API / Web

- `ElliottDetectorResult.tez_ew` → `annotations.elliott.tez_ew` (`TezElliottEwSnapshot`)
  - `TezImpulseRules`: `ImpulseValidation` alanlarından (`TezImpulseRules::from_validation`)
  - `TezZigzagRules`: ABC fiyatlarından (`TezZigzagRules::from_abc_prices`) — zigzag panelinde
  - `nested_wave_hint`: iç dalga (`subwave_validation`, `corr_subwave_validation`) açıklaması
- Web: **Elliott Wave** paneli → **tez** kutusu (`#ewTezEw`), `formatTezEwPanel()` ile satır satır ✓/✗.

## Kapsam notu

- **Yassı / üçgen / kombinasyon:** `flat_valid_detailed`, `try_triangle`, vb. mevcut kurallar; tez snapshot’ta flat için yalnızca `source` + `nested_wave_hint` (zigzag alanı boş) verilebilir.
