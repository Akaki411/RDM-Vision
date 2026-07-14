# RDM-Vision

![Rust](https://img.shields.io/badge/Rust-1.95.0-DA5929)
![Docker](https://img.shields.io/badge/docker-257bd6?style=for-the-badge&logo=docker&logoColor=white)

Программа распознавания **DataMatrix** кодов с видеопотока RTSP и GigE Vision / GenICam камер.
Построена на open-source решениях: [YOLO26-pose](https://github.com/ultralytics/ultralytics), [rxing](https://github.com/rxing-core/rxing), [Irec](https://github.com/big-yogurt/irec)

---

## Подготовка

Для нативной компиляции программы потребуется:
- [rustup](https://rustup.rs/)

Возможна и сборка через Docker

---

## Нативная сборка и запуск
Только release (из-за особенностей ONNX-runtime):
```sh
cargo run --release
```
Собрать бинарник:
```sh
cargo build --release
```

## Сборка и запуск через Docker:

Собрать образ:
```sh
docker build -t rdm-vision .
```
Запустить контейнер:
```sh
docker run --rm -it --network host rdm-vision
```

---

## Настройки

Настройки сервиса хранятся в файле config.json, при первом запуске он создается со стандартными значениями. Вот назначение параметров:

``` json
{
  "cameras": [],                    // массив камер, формат записи зависит от "type", см. ниже
  "normalization":                  // параметры нормализации перед обработкой
  {
    "enabled": "bool",              // включение нормализации
    "target_size": "int",           // размер большей стороны целевого кадра
    "grayscale": "bool",            // переводить ли кадр в ч/б
    "contrast": "float"             // верхний предел усиления контраста
  },
  "detection":                      // настройки распознавания области кода
  {
    "model_path": "string",         // путь к ONNX модели YOLO
    "input_size": "int",            // размер входного тензора модели
    "confidence_threshold": "float",// порог уверенности детекции
    "nms_threshold": "float",       // порог подавления пересекающихся детекций
    "blur_threshold": "float"       // порог дисперсии Лапласиана, ниже - область считается смазанной
  },
  "recognition":                    // параметры декодера rxing
  {
    "try_harder": "bool",           // тратить больше времени ради точности, а не скорости
    "try_rotate": "bool",           // пробовать повёрнутые на 90/180/270° варианты кадра
    "try_invert": "bool",           // пробовать инвертированное изображение
    "try_downscale": "bool"         // пробовать уменьшенный вдвое вариант кадра
  },
  "restore_service":                // настройка сервиса восстановления Irec
  {
    "enabled": "bool",              // включение сервиса восстановления
    "endpoint": "string",           // адрес gRPC сервиса восстановления кода
    "timeout_ms": "int"             // таймаут запроса к сервису восстановления
  },
  "websocket":                      // настройка WebSocket сервера рассылки кодов
  {
    "port": "int",                  // порт WebSocket сервера
    "repeat_time_ms": "int"         // блокировка повторной рассылки одного и того же кода
  },
  "pipeline":                       // настройка самого пайплайна
  {                                 
    "cold_fps": "float",            // предел кадров/сек, пока код не виден в кадре (холодный ход)
    "hot_fps": "float",             // предел кадров/сек, когда код в кадре виден (разгон)
    "hot_hold_ms": "int"            // сколько держим горячий режим после последнего обнаружения кода
  },
  "preview": "bool"                 // показывать окно предпросмотра потока (для отладки)
}
```

### Формат записи камер

Тип камеры задаётся полем `"type"`. Общих полей у RTSP и GigE нет — набор параметров зависит от типа.

**RTSP IP камера:**

```json
{
  "type": "rtsp",
  "id": "cam-rtsp-01",
  "url": "rtsp://localhost:8554/",
  "fps": 25.0,
  "transport": "tcp",
  "reconnect_delay_ms": 2000,
  "read_timeout_ms": 5000,
  "enabled": true
}
```


**GigE Vision / GenICam камера:**:

```json
{
  "type": "gige",
  "id": "cam-gige-01",
  "address": "127.0.0.1",
  "interface": "127.0.0.1",
  "enabled": true
}
```

---

## Вывод результатов

Распознанные коды рассылаются по WebSocket всем подключённым клиентам (`ws://<host>:<port>`, порт задаётся в `websocket.port`).
При каждом сканировании клиент получает JSON-объект:

```json
{
  "camera_id": "string",  // идентификатор камеры
  "code": "string",       // содержимое DataMatrix
  "restored": false,      // true, если код был восстановлен
  "time_ms": 0            // время обработки кадра до распознавания, мс
}
```

---

## Тестирование
Для тестирования обработки рекомендуется воспользоваться эмулятором RTSP камеры [RTSP-emu](https://github.com/Akaki411/RTSP-emu) путем загрузки в него видеофайла с DataMatrix кодами.