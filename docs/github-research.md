# Benzer açık kaynak uygulamalar — 2026-10-19

- OpenRGB: https://github.com/CalcProgrammer1/OpenRGB
  Ana README: renk/efekt kontrolü, profiller, CLI, SDK ve grafik LED görünümü.
  Bizim panelde cihaz kartı ve yerel profil kaydet/yükle akışına örnek.
- OpenRazer: https://github.com/openrazer/openrazer
  Sürücü + DBus + kullanıcı arayüzü ayrımı. Desteklenen Razer cihazları VID
  1532 kullanıyor; Lecoo 1a2c:9df4 için bu sürücüyü kurmak çözüm sayılmaz.
- Polychromatic: https://github.com/polychromatic/polychromatic
  OpenRazer üzerine aydınlatma ve cihaz özellikleri arayüzü. Panelin
  donanımın gerçekten sunduğu yeteneklere göre kontrol göstermesine örnek.
- rgb_keyboard: https://github.com/dokutan/rgb_keyboard
  Bazı 0c45 VID klavyelerde özel tuş renkleri ve profiller. Bu modelin
  protokolüyle eşleşme yok; paketlerini Lecoo'ya göndermedik.
- Aynı üreticiye ait farklı PID için protokol araştırması:
  https://gitlab.com/CalcProgrammer1/OpenRGB/-/issues/2470
  Redragon 1a2c:4b4d örneği 07 F7 komut ailesi ve sabit renk kodları
  kullanıyor. Lecoo'nun kanıtlı 07 FF FF ailesinden farklı. Ortak VID,
  komutların uyumlu olduğunu kanıtlamaz.

Arama sınırı: GitHub/OpenRGB üzerinde Lecoo/GK301/9df4/Shinetek ve bilinen
Linux RGB yönetim projeleri. Bu aramada GK301 için doğrulanmış serbest RGB
sürücüsü bulunmadı; bu sonuç dünyada hiç olmadığı iddiası değildir.

Kaynak kod kopyalanmadı; genel arayüz yaklaşımı ve yetenek modeli incelendi.
