# Bağlı GK301: canlı USB kaydı ve REA analizi

2026-10-19. Cihaz VID:PID 1a2c:9df4, USB bus8/device7; hidraw5 vendor
interface1. Firmware sorgu yanıtı 07 01 16 00 00 00 00 00 (1.16).

## Doğrudan donanım gözlemi

Wireshark CLI4.7.4 kuruldu ve Linux usbmon modülü yüklendi. dumpcap ile
15 saniyelik, yalnız bu cihazın CONTROL URB'lerini içeren kayıt alındı:
`link[9] = 2 and link[11] = 7`. 16 kayıt, 0 kayıp bildirildi. Interrupt
klavye girdileri filtre dışındadır. Kayıt sona erdi; arka planda yakalama yok.

Bu süre içinde firmware sorgusu, mevcut Breathing ayarının tekrar gönderimi
ve ikinci firmware sorgusu yapıldı. Mevcut efekt değiştirilmedi.

REA6.7.0 CLI, açıkça REA_TSHARK_COMMAND=/usr/bin/tshark seçilerek bu gerçek
kaydı çözümledi; MCP sunucu konfigürasyonu değiştirilmedi.

REA Evidence: ev_1e20977fbfca2c4906f3cebd2230dd6062c5378f9ca0083fde0886fcf4a61e1c
Tam kayıt: live-control.pcapng
REA çıktısı: rea-live-control.json
USB/HID envanteri: live-hid-inventory.json

| Kayıt | Gözlem |
|---|---|
|7–8|SET_REPORT, wValue0307, interface1, 8 bayt: 07 F1 00 00 00 00 00 00; completion status0|
|9–10|GET_REPORT Feature7, interface1; gerçek cihaz yanıtı 07 01 16 00 00 00 00 00|
|11–12|SET_REPORT Feature7, interface1; 07 FF FF 02 05 00 00 00; completion status0|
|13–16|İkinci firmware sorgusu ve aynı firmware yanıtı|

Dolayısıyla Linux komutu gerçekten USB interface1'e Feature report7 olarak
gidiyor; veri uzunluğu8. Bu artık yalnız ioctl dönüş değerine dayanmıyor.

## Sınırlar

REA'nın kendisi canlı USB yakalamaz; donanım kaydı dumpcap tarafından
alındı, dosyası REA ile çözümlendi. Bu bir firmware dökümü/decompilation
veya LED renginin optik ölçümü değildir. Ek komut, özel RGB veya yan LED
kabiliyeti bu kayıttan çıkarılamaz. Donanım hatası veya yeni özellik bulunduğu
iddia edilmez. Yeni rastgele komut gönderilmedi.
