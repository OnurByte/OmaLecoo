# Fiziksel kullanıcı doğrulaması — 2026-10-19

Önceki transfer testlerinin rc=8 sonucu LED'in değiştiğini kanıtlamıyordu.
Bu turdaki fiziksel gözlem, doğrudan kullanıcının yanıtlarıyla kaydedildi:

1. Ayrı CLI çağrılarıyla panel0/parlaklık5 ve side3/parlaklık7 Feature:
   kullanıcı hiçbir değişiklik olmadığını bildirdi.
2. Aynı iki bilinen ışık paketi Output: kullanıcı değişiklik olmadığını bildirdi.
3. Aynı açık handle üzerinde F1 Set/Get, 150ms aralıklarla aynı panel/yan
   paketleri: kullanıcı kısa titreme ve önceki duruma dönüş bildirdi.
4. Windows'tan kurtarılan gecikmeli başlangıç yolu: aynı açık handle F1
   Set/Get, 2 saniye bekle, `07 FF FF 00 0D 00 00 00` Feature, 2 saniye
   açık tut. Kullanıcı bu kez kalıcı değişiklik olduğunu bildirdi.

Son deney hem bekleme süresini hem brightness alanını değiştirdi. Bu yüzden
nedenin yalnızca gecikme, 0D değeri veya bağlantının açık kalması olduğu
iddia edilemez. Henüz otomatik yeni bir aydınlatma protokolü eklenmedi.

Kullanıcı, mevcut efekt kontrollerinin çalıştığını ayrıca belirtti. Tüm
kontrolleri kaldırmak bu geri bildirimle kapsam dışına çıktı ve geri alındı.
Efektler, parlaklık, mevcut yan efekt listesi ve profiller geri yüklendi.
Sadece üstteki dekoratif klavye çizimi ve başarısı doğrulanmamış yeni renk
kısayolları kaldırıldı. Önceki UI archive/0.4.0-plugin altında korunuyor.

Kalan ayrım: mevcut efekt komutlarını, gecikmeli başlangıç paketinin byte4
0D değeri ve fiziksel etkisiyle karşılaştırmak; gerçek cihaz LED durumunun
okunup okunamadığını belirlemek. Rastgele ham komut taraması yapılmadı.

## Son kullanıcı doğrulaması ve model sınırı

Kullanıcı geri yüklenen ana efekt seçicisinin fiziksel klavyeyi değiştirdiğini,
yan LED kontrollerinin ise tepki vermediğini doğruladı.

Yeni XML bulgusu: dlg_main.xml 15–124 arası Keyboard sayfasıdır; Panel LED
buradadır. Side LED ise 125. satırdaki Mouse sayfasının 126. satırında
visible=0 ile yer alır. Bu, GK301 yan LED desteğini göstermeyen genel uygulama
kodu olduğunu güçlü biçimde düşündürür; kesin firmware yokluğu iddiası değildir.
Yan LED UI'si kaldırıldı; ana efektler korundu. UI profil-load/preset/restore
komutları --panel-only kullanır, bu yollardan side paketi çıkmaz. Legacy
araştırma CLI'si ve referanslar korunur.

Makro araştırması: 420580 işçisi 420621 adresinde Windows keybd_event çağırır;
makrolar bilgisayarda yürütülür. Dahili klavye hafızasına makro yazma protokolü
çıkarılmış sayılmaz. Bu nedenle on-device makro özelliği eklenmedi.

scripts/probe-windows-startup.py varsayılan salt gösterim modundadır;
--send verilirse önceki fiziksel deneyi aynı açık bağlantıda tekrarlar.
Son paketin byte4 değeri0D ve gecikme aynı deneyde değiştiğinden bunların
tekil etkisi hâlâ ayrıştırılmadı.
