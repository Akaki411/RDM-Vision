# RDM-Vision

![Rust](https://img.shields.io/badge/Rust-1.95.0-DA5929)
![CMake](https://img.shields.io/badge/CMake-064F8C?logo=cmake&logoColor=fff)
![Docker](https://img.shields.io/badge/docker-257bd6?style=for-the-badge&logo=docker&logoColor=white)

Программа распознавания **DataMatrix** кодов с видеопотока RTSP и GigE Vision / GenICam камер.
Построена на open-source решениях: [YOLO26-pose](https://github.com/ultralytics/ultralytics), [ZXing-cpp](https://github.com/zxing-cpp/zxing-cpp), [Irec](https://github.com/big-yogurt/irec)

---

## Подготовка

Для нативной компиляции программы потребуется:
- [rustup](https://rustup.rs/)
- [CMake](https://cmake.org/download/)

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
  "restore_service":                // настройка сервиса восстановления Irec
  {
    "enabled": "bool",              // включение сервиса восстановления
    "endpoint": "string",           // адрес gRPC сервиса восстановления кода
    "timeout_ms": "int"             // таймаут запроса к сервису восстановления
  },
  "api":                            // настройка взаимодкйствия с API сервером
  {
    "base_url": "string",           // адрес сервера приёма кодов
    "code_endpoint": "string",      // путь для отправки распознанного кода
    "repeat_time_ms": "int",        // блокировка повторной отправки одного и того же кода
    "timeout_ms": "int"             // таймаут запроса к серверу
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

## Тестирование
Для тестирования обработки рекомендуется воспользоваться эмулятором RTSP камеры [RTSP-emu](https://github.com/Akaki411/RTSP-emu) путем загрузки в него видеофайла с DataMatrix кодами.