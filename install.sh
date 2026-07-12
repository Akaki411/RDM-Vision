#!/bin/bash

# Проверка рута
if [ "$EUID" -ne 0 ]; then
  echo "Warning: sudo required"
  exit 1
fi

echo "Starting RDM-Vision configuration..."

# Создание пользователя на хосте, если его еще нет
if id "rdmv" &>/dev/null; then
    echo "The user rdmv already exists"
else
    echo "Creating the system user rdmv..."
    useradd -r -s /bin/false rdmv
    # Добавление в группу докера
    usermod -aG docker rdmv
fi

PROJECT_DIR=$(pwd)

# Создание папки для монтирования, чтобы Докер не создал их от рута
mkdir -p "$PROJECT_DIR/config" "$PROJECT_DIR/models"

# Выдача прав пользователю rdmv на директорию проекта
echo "Permissions granted to user rdmv for the directory: $PROJECT_DIR"
chown -R rdmv:rdmv "$PROJECT_DIR"

# создание службы для systemd
SERVICE_FILE="/etc/systemd/system/rdm-vision.service"
echo "Creating a systemd ($SERVICE_FILE) service..."

cat > "$SERVICE_FILE" <<EOF
[Unit]
Description=RDM-Vision Service
Requires=docker.service
After=docker.service
# Лимит перезапусков по времени (секунды)
StartLimitIntervalSec=60
# Лимит кол-ва перезапусков
StartLimitBurst=5

[Service]
Type=simple
User=rdmv
Group=rdmv
WorkingDirectory=$PROJECT_DIR
ExecStartPre=/usr/bin/docker compose -f Dockerfile-compose down
ExecStart=/usr/bin/docker compose -f Dockerfile-compose up
ExecStop=/usr/bin/docker compose -f Dockerfile-compose down

# Авторестарт контейнера при падении
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

# запуск
echo "Starting the service..."
systemctl daemon-reload
systemctl enable rdm-vision
systemctl restart rdm-vision

echo "Done"