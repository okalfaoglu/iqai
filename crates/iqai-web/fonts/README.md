# Telegram kart fontu (CANLI POZİSYON / pozisyon açılışı)

PNG kartları `rusttype` ile çizer; **bir TTF font gerekir**.

1. **Önerilen (repo içi):** bu klasöre `DejaVuSans.ttf` koyun:
   ```bash
   cd /path/to/iqai
   curl -fsSL -o crates/iqai-web/fonts/DejaVuSans.ttf \
     "https://raw.githubusercontent.com/dejavu-fonts/dejavu-fonts/master/ttf/DejaVuSans.ttf"
   ```
2. **Sistem:** Ubuntu/Debian’da genelde `/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf` (paket: `fonts-dejavu-core`).
3. **Ortam değişkeni:** `IQAI_FONT_PATH=/path/to/font.ttf`

Font yoksa kart PNG üretilemez; Telegram’a yapılandırılmış **HTML metin** yedeği gider.
