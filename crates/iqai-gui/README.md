# iqai-gui (masaüstü)

Bu crate **şimdilik bir yer tutucudur**; grafik için `iqai-web` (tarayıcı) veya CLI kullanılır.

- **Durum:** `src/main.rs` yalnızca bilgi mesajı yazdırır.
- **Neden workspace’te yok:** Varsayılan kök `Cargo.toml` workspace üyeleri masaüstü (winit/egui vb.) gerektirmediği için `iqai-gui` hariç tutulur; tam derleme için `members` listesine `"crates/iqai-gui"` ekleyin.
- **Bağımlılıklar (ileride):** display ortamı — Linux X11/Wayland, Windows, macOS.

## Karar özeti

| Seçenek | Not |
|---------|-----|
| **A — Web öncelik** | `iqai-web` zaten chart + paneller; çoğu kullanım için yeterli. |
| **B — Yerel GUI** | `iqai-gui`: Rust + winit/egui veya Tauri (web view içinde `iqai-web`). |

**Öneri:** Önce A; yerel pencere gerekiyorsa Tauri ile `iqai-web`’i sarın (tek kod tabanı).

## Sonraki adımlar (isteğe bağlı)

1. Tauri iskeleti + `iqai-web`’i `devUrl` ile bağlama.
2. Veya doğrudan `winit` + `egui` ile mum grafiği (yüksek efor).
