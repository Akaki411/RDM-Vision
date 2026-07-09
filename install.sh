#!/bin/bash

# Проверка рута
if [ "$EUID" -ne 0 ]; then
  echo "Warning: sudo required"
  exit 1
fi

echo "Starting RDM-Vision configuration..."

# Создание пользователя, если его еще нет в системе
if id "rdmv" &>/dev/null; then
    echo "The user rdmv already exists"
else
    echo "Creating the system user rdmv..."
    useradd -r -s /bin/false rdmv
    # Добавление в группу докера
    usermod -aG docker rdmv
fi

# Поиск пути проекта и выдача прав
PROJECT_DIR=$(pwd)
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

[Service]
Type=simple
User=rdmv
Group=rdmv
WorkingDirectory=$PROJECT_DIR
ExecStartPre=/usr/bin/docker compose down
ExecStart=/usr/bin/docker compose up
ExecStop=/usr/bin/docker compose down

Restart=on-failure
RestartSec=5
# защита от бесконечного цикла

# колво перезапусков
StartLimitIntervalSec=60

# за период времени
StartLimitBurst=5

[Install]
WantedBy=multi-user.target
EOF

# запуск
echo "Starting the service..."
systemctl daemon-reload
systemctl enable rdm-vision
systemctl restart rdm-vision

echo "Done"