// ============================================================================
// DATABASE MODULE - SQLite Datenbankintegration für User-Management & Device-Management
// ============================================================================

use sqlx::{sqlite::SqlitePool, Row};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use bcrypt::{hash, verify, DEFAULT_COST};
use serde::{Deserialize, Serialize};
use std::fs;

// ============================================================================
// DATABASE STRUCTS
// ============================================================================

#[derive(Debug, Deserialize, Serialize)]
pub struct InitialUserConfig {
    pub email: String,
    pub display_name: String,
    pub password: String,
    pub is_admin: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct InitialUsersFile {
    pub users: Vec<InitialUserConfig>,
}

#[derive(Debug, Clone)]
pub struct DatabaseUser {
    pub id: String,
    pub email: String,
    pub display_name: String,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
    pub is_admin: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Device {
    pub mac_address: String, // Primary key - moved to first position
    pub name: String,
    pub alias: Option<String>, // User-defined display name
    pub owner_id: String,
    pub ip_address: Option<String>,
    pub status: DeviceStatus,
    pub maintenance_mode: bool,
    pub firmware_version: Option<String>,
    pub last_seen: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub connection_type: String, // "tcp" or "uart"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceStatus {
    Online,
    Offline,
    Error,
    Updating,
    Maintenance,
}

#[derive(Debug, Clone, Serialize)]
pub struct DevicePermission {
    pub device_id: String,
    pub user_id: String,
    pub permission: String,
}

impl DatabaseUser {
    pub fn new(email: String, display_name: String, password: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let password_hash = hash(password, DEFAULT_COST)?;
        Ok(Self {
            id: Uuid::new_v4().to_string(),
            email,
            display_name,
            password_hash,
            created_at: Utc::now(),
            is_admin: false,
        })
    }

    pub fn verify_password(&self, password: &str) -> Result<bool, bcrypt::BcryptError> {
        verify(password, &self.password_hash)
    }
}

impl Device {
    pub fn new(name: String, owner_id: String, mac_address: String) -> Self {
        let now = Utc::now();
        Self {
            mac_address, // Primary key
            name,
            alias: None, // Initially no alias set
            owner_id,
            ip_address: None,
            status: DeviceStatus::Offline,
            maintenance_mode: false,
            firmware_version: None,
            last_seen: now,
            created_at: now,
            connection_type: "tcp".to_string(), // Default to TCP
        }
    }

    pub fn new_uart(name: String, owner_id: String, mac_address: String) -> Self {
        let now = Utc::now();
        Self {
            mac_address,
            name,
            alias: None, // Initially no alias set
            owner_id,
            ip_address: None,
            status: DeviceStatus::Offline,
            maintenance_mode: false,
            firmware_version: None,
            last_seen: now,
            created_at: now,
            connection_type: "uart".to_string(),
        }
    }

    pub fn update_status(&mut self, status: DeviceStatus, ip_address: Option<String>) {
        self.status = status;
        self.ip_address = ip_address;
        self.last_seen = Utc::now();
    }
}

// ============================================================================
// DATABASE MANAGER
// ============================================================================

#[derive(Debug)]
pub struct DatabaseManager {
    pool: SqlitePool,
}

impl DatabaseManager {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Erstelle SQLite-Datenbankdatei wenn sie nicht existiert
        std::fs::create_dir_all("data").ok();
        
        let database_url = "sqlite:data/users.db?mode=rwc";
        let pool = SqlitePool::connect(database_url).await?;
        
        let db_manager = Self { pool };
        
        // Tabellen erstellen
        db_manager.init_database().await?;
        
        // Initiale User aus Konfiguration erstellen
        db_manager.create_initial_users().await?;
        
        Ok(db_manager)
    }

    async fn init_database(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Foreign Keys aktivieren (SQLite erzwingt diese standardmäßig NICHT)
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&self.pool)
            .await?;

        // Users Tabelle erstellen
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                email TEXT UNIQUE NOT NULL,
                display_name TEXT NOT NULL,
                password_hash TEXT NOT NULL,
                created_at TEXT NOT NULL,
                is_admin BOOLEAN NOT NULL DEFAULT FALSE
            )
            "#
        )
        .execute(&self.pool)
        .await?;

        // Create system "guest" user with fixed ID (required for FOREIGN KEY constraints)
        // This user is independent from initial_users.json and cannot be deleted
        sqlx::query(
            "INSERT OR IGNORE INTO users (id, email, display_name, password_hash, created_at, is_admin) VALUES ('guest', 'guest@system.local', 'Guest User', '', ?, FALSE)"
        )
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;

        // Devices Tabelle erstellen
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS devices (
                mac_address TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                owner_id TEXT NOT NULL,
                ip_address TEXT,
                status TEXT NOT NULL DEFAULT 'Offline',
                maintenance_mode BOOLEAN NOT NULL DEFAULT FALSE,
                firmware_version TEXT,
                last_seen TEXT NOT NULL,
                created_at TEXT NOT NULL,
                connection_type TEXT NOT NULL DEFAULT 'tcp',
                FOREIGN KEY (owner_id) REFERENCES users (id)
            )
            "#
        )
        .execute(&self.pool)
        .await?;

        // DEVICE PERMISSIONS Tabelle erstellen
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS device_permissions (
                device_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                permission TEXT NOT NULL,
                PRIMARY KEY (device_id, user_id),
                FOREIGN KEY (device_id) REFERENCES devices (mac_address),
                FOREIGN KEY (user_id) REFERENCES users (id)
            )
            "#
        )
        .execute(&self.pool)
        .await?;

        // UART Settings Tabelle erstellen
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS uart_settings (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                port TEXT,
                baud_rate INTEGER NOT NULL DEFAULT 115200,
                auto_connect BOOLEAN NOT NULL DEFAULT FALSE,
                updated_at TEXT NOT NULL
            )
            "#
        )
        .execute(&self.pool)
        .await?;

        // Insert default UART settings if not exists
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO uart_settings (id, port, baud_rate, auto_connect, updated_at)
            VALUES (1, NULL, 115200, FALSE, datetime('now'))
            "#
        )
        .execute(&self.pool)
        .await?;

        // Debug Settings Tabelle erstellen
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS debug_settings (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                max_debug_messages INTEGER NOT NULL DEFAULT 200,
                updated_at TEXT NOT NULL
            )
            "#
        )
        .execute(&self.pool)
        .await?;

        // Insert default Debug settings if not exists
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO debug_settings (id, max_debug_messages, updated_at)
            VALUES (1, 200, datetime('now'))
            "#
        )
        .execute(&self.pool)
        .await?;

        // Migration: Add connection_type column if it doesn't exist (for existing databases)
        let migration_result = sqlx::query(
            r#"
            ALTER TABLE devices ADD COLUMN connection_type TEXT NOT NULL DEFAULT 'tcp'
            "#
        )
        .execute(&self.pool)
        .await;

        // Ignore error if column already exists
        match migration_result {
            Ok(_) => tracing::info!("Database migration: Added connection_type column to devices"),
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("duplicate column") || error_msg.contains("already exists") {
                    tracing::debug!("Database migration: connection_type column already exists");
                } else {
                    tracing::warn!("Database migration warning: {}", error_msg);
                }
            }
        }

        // Migration: Add alias column if it doesn't exist (for existing databases)
        let migration_result = sqlx::query(
            r#"
            ALTER TABLE devices ADD COLUMN alias TEXT
            "#
        )
        .execute(&self.pool)
        .await;

        // Ignore error if column already exists
        match migration_result {
            Ok(_) => tracing::info!("Database migration: Added alias column to devices"),
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("duplicate column") || error_msg.contains("already exists") {
                    tracing::debug!("Database migration: alias column already exists");
                } else {
                    tracing::warn!("Database migration warning: {}", error_msg);
                }
            }
        }

        Ok(())
    }

    pub async fn create_user(&self, user: DatabaseUser) -> Result<(), Box<dyn std::error::Error>> {
        sqlx::query(
            "INSERT INTO users (id, email, display_name, password_hash, created_at, is_admin) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(&user.id)
        .bind(&user.email)
        .bind(&user.display_name)
        .bind(&user.password_hash)
        .bind(user.created_at.to_rfc3339())
        .bind(user.is_admin)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_user_by_email(&self, email: &str) -> Result<Option<DatabaseUser>, Box<dyn std::error::Error>> {
        let row = sqlx::query("SELECT * FROM users WHERE email = ?")
            .bind(email)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(row) => {
                let created_at_str: String = row.get("created_at");
                let created_at = DateTime::parse_from_rfc3339(&created_at_str)?.with_timezone(&Utc);
                
                Ok(Some(DatabaseUser {
                    id: row.get("id"),
                    email: row.get("email"),
                    display_name: row.get("display_name"),
                    password_hash: row.get("password_hash"),
                    created_at,
                    is_admin: row.get("is_admin"),
                }))
            }
            None => Ok(None)
        }
    }

    pub async fn get_user_by_id(&self, user_id: &str) -> Result<Option<DatabaseUser>, Box<dyn std::error::Error>> {
        let row = sqlx::query("SELECT * FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(row) => {
                let created_at_str: String = row.get("created_at");
                let created_at = DateTime::parse_from_rfc3339(&created_at_str)?.with_timezone(&Utc);
                
                Ok(Some(DatabaseUser {
                    id: row.get("id"),
                    email: row.get("email"),
                    display_name: row.get("display_name"),
                    password_hash: row.get("password_hash"),
                    created_at,
                    is_admin: row.get("is_admin"),
                }))
            }
            None => Ok(None)
        }
    }

    pub async fn update_user_display_name(&self, user_id: &str, display_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        sqlx::query("UPDATE users SET display_name = ? WHERE id = ?")
            .bind(display_name)
            .bind(user_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn get_all_users(&self) -> Result<Vec<DatabaseUser>, Box<dyn std::error::Error>> {
        let rows = sqlx::query("SELECT * FROM users ORDER BY created_at DESC")
            .fetch_all(&self.pool)
            .await?;

        let mut users = Vec::new();
        for row in rows {
            let created_at_str: String = row.get("created_at");
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)?.with_timezone(&Utc);
            
            users.push(DatabaseUser {
                id: row.get("id"),
                email: row.get("email"),
                display_name: row.get("display_name"),
                password_hash: row.get("password_hash"),
                created_at,
                is_admin: row.get("is_admin"),
            });
        }

        Ok(users)
    }

    pub async fn search_users(&self, query: &str) -> Result<Vec<DatabaseUser>, Box<dyn std::error::Error>> {
        let search_pattern = format!("%{}%", query);
        let rows = sqlx::query("SELECT * FROM users WHERE email LIKE ? OR display_name LIKE ? ORDER BY display_name LIMIT 20")
            .bind(&search_pattern)
            .bind(&search_pattern)
            .fetch_all(&self.pool)
            .await?;

        let mut users = Vec::new();
        for row in rows {
            let created_at_str: String = row.get("created_at");
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)?.with_timezone(&Utc);
            
            users.push(DatabaseUser {
                id: row.get("id"),
                email: row.get("email"),
                display_name: row.get("display_name"),
                password_hash: row.get("password_hash"),
                created_at,
                is_admin: row.get("is_admin"),
            });
        }

        Ok(users)
    }

    pub async fn get_users_paginated(&self, offset: i32, limit: i32) -> Result<Vec<DatabaseUser>, Box<dyn std::error::Error>> {
        let rows = sqlx::query("SELECT * FROM users ORDER BY display_name LIMIT ? OFFSET ?")
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

        let mut users = Vec::new();
        for row in rows {
            let created_at_str: String = row.get("created_at");
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)?.with_timezone(&Utc);
            
            users.push(DatabaseUser {
                id: row.get("id"),
                email: row.get("email"),
                display_name: row.get("display_name"),
                password_hash: row.get("password_hash"),
                created_at,
                is_admin: row.get("is_admin"),
            });
        }

        Ok(users)
    }

    pub async fn delete_user(&self, user_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Zuerst Permissions löschen
        sqlx::query("DELETE FROM device_permissions WHERE user_id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?;

        // Devices des Users auf Guest übertragen (FK-Constraint: owner_id muss existieren)
        sqlx::query("UPDATE devices SET owner_id = 'guest' WHERE owner_id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?;

        // Dann User löschen
        sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn update_user_admin_status(&self, user_id: &str, is_admin: bool) -> Result<(), Box<dyn std::error::Error>> {
        sqlx::query("UPDATE users SET is_admin = ? WHERE id = ?")
            .bind(is_admin)
            .bind(user_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    // ============================================================================
    // INITIAL USERS MANAGEMENT - Lädt und erstellt initiale User aus Konfiguration
    // ============================================================================

    fn load_initial_users() -> Result<InitialUsersFile, Box<dyn std::error::Error>> {
        let config_path = "data/initial_users.json";
        
        if !std::path::Path::new(config_path).exists() {
            tracing::warn!("Initial users config file not found: {}", config_path);
            // Fallback zu Standard Admin-User
            return Ok(InitialUsersFile {
                users: vec![InitialUserConfig {
                    email: "admin@drawing-app.local".to_string(),
                    display_name: "Administrator".to_string(),
                    password: "admin123".to_string(),
                    is_admin: true,
                }],
            });
        }

        let config_content = fs::read_to_string(config_path)?;
        let config: InitialUsersFile = serde_json::from_str(&config_content)?;
        
        tracing::info!("Loaded {} initial users from config", config.users.len());
        Ok(config)
    }

    async fn create_initial_users(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Prüfen ob bereits User existieren
        let user_count = sqlx::query("SELECT COUNT(*) as count FROM users")
            .fetch_one(&self.pool)
            .await?
            .get::<i64, _>("count");

        if user_count > 0 {
            tracing::info!("Database contains {} existing users, skipping initial user creation", user_count);
            return Ok(());
        }

        // Initiale User aus Konfiguration laden
        let config = Self::load_initial_users()?;
        let mut created_count = 0;

        for user_config in config.users {
            tracing::debug!("Creating initial user: {}", user_config.email);
            
            let db_user = DatabaseUser {
                id: Uuid::new_v4().to_string(),
                email: user_config.email.clone(),
                display_name: user_config.display_name,
                password_hash: hash(&user_config.password, DEFAULT_COST)?,
                created_at: Utc::now(),
                is_admin: user_config.is_admin,
            };

            match self.create_user(db_user).await {
                Ok(_) => {
                    created_count += 1;
                    if user_config.is_admin {
                        tracing::info!("Created initial admin user: {} / {}", user_config.email, user_config.password);
                    } else {
                        tracing::info!("Created initial user: {} / {}", user_config.email, user_config.password);
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to create initial user {}: {:?}", user_config.email, e);
                }
            }
        }

        if created_count > 0 {
            tracing::info!("Successfully created {} initial users", created_count);
        }

        Ok(())
    }

    // ============================================================================
    // DEVICE MANAGEMENT - CRUD Operationen für Devices
    // ============================================================================

    pub async fn create_device(&self, device: Device) -> Result<(), Box<dyn std::error::Error>> {
        let status_str = match device.status {
            DeviceStatus::Online => "Online",
            DeviceStatus::Offline => "Offline", 
            DeviceStatus::Error => "Error",
            DeviceStatus::Updating => "Updating",
            DeviceStatus::Maintenance => "Maintenance",
        };
        
        sqlx::query(
            "INSERT INTO devices (mac_address, name, alias, owner_id, ip_address, status, maintenance_mode, firmware_version, last_seen, created_at, connection_type) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&device.mac_address)
        .bind(&device.name)
        .bind(&device.alias)
        .bind(&device.owner_id)
        .bind(&device.ip_address)
        .bind(status_str)
        .bind(device.maintenance_mode)
        .bind(&device.firmware_version)
        .bind(device.last_seen.to_rfc3339())
        .bind(device.created_at.to_rfc3339())
        .bind(&device.connection_type)
        .execute(&self.pool)
        .await?;

        // Owner-Berechtigung hinzufügen
        self.set_device_permission(&device.mac_address, &device.owner_id, "O").await?;

        Ok(())
    }

    pub async fn get_device_by_id(&self, device_id: &str) -> Result<Option<Device>, Box<dyn std::error::Error>> {
        let row = sqlx::query("SELECT * FROM devices WHERE mac_address = ?")
            .bind(device_id)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(row) => {
                let created_at_str: String = row.get("created_at");
                let created_at = DateTime::parse_from_rfc3339(&created_at_str)?.with_timezone(&Utc);
                let last_seen_str: String = row.get("last_seen");
                let last_seen = DateTime::parse_from_rfc3339(&last_seen_str)?.with_timezone(&Utc);
                
                let status_str: String = row.get("status");
                let status = match status_str.as_str() {
                    "Online" => DeviceStatus::Online,
                    "Offline" => DeviceStatus::Offline,
                    "Error" => DeviceStatus::Error,
                    "Updating" => DeviceStatus::Updating,
                    "Maintenance" => DeviceStatus::Maintenance,
                    _ => DeviceStatus::Offline,
                };
                
                Ok(Some(Device {
                    mac_address: row.get("mac_address"),
                    name: row.get("name"),
                    alias: row.try_get::<Option<String>, _>("alias").unwrap_or(None),
                    owner_id: row.get("owner_id"),
                    ip_address: row.get("ip_address"),
                    status,
                    maintenance_mode: row.get("maintenance_mode"),
                    firmware_version: row.get("firmware_version"),
                    last_seen,
                    created_at,
                    connection_type: row.try_get("connection_type").unwrap_or_else(|_| "tcp".to_string()),
                }))
            }
            None => Ok(None)
        }
    }

    pub async fn list_user_devices(&self, user_id: &str) -> Result<Vec<(Device, String)>, Box<dyn std::error::Error>> {
        let rows = sqlx::query(
            r#"
            SELECT d.*, dp.permission
            FROM devices d
            INNER JOIN device_permissions dp ON d.mac_address = dp.device_id
            WHERE dp.user_id = ?
            ORDER BY d.created_at DESC
            "#
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        let mut device_list = Vec::new();
        for row in rows {
            let created_at_str: String = row.get("created_at");
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)?.with_timezone(&Utc);
            let last_seen_str: String = row.get("last_seen");
            let last_seen = DateTime::parse_from_rfc3339(&last_seen_str)?.with_timezone(&Utc);
            
            let status_str: String = row.get("status");
            let status = match status_str.as_str() {
                "Online" => DeviceStatus::Online,
                "Offline" => DeviceStatus::Offline,
                "Error" => DeviceStatus::Error,
                "Updating" => DeviceStatus::Updating,
                "Maintenance" => DeviceStatus::Maintenance,
                _ => DeviceStatus::Offline,
            };

            let device = Device {
                mac_address: row.get("mac_address"),
                name: row.get("name"),
                alias: row.try_get::<Option<String>, _>("alias").unwrap_or(None),
                owner_id: row.get("owner_id"),
                ip_address: row.get("ip_address"),
                status,
                maintenance_mode: row.get("maintenance_mode"),
                firmware_version: row.get("firmware_version"),
                last_seen,
                created_at,
                connection_type: row.try_get("connection_type").unwrap_or_else(|_| "tcp".to_string()),
            };

            let permission: String = row.get("permission");
            device_list.push((device, permission));
        }

        Ok(device_list)
    }

    pub async fn list_all_devices(&self) -> Result<Vec<Device>, Box<dyn std::error::Error>> {
        let rows = sqlx::query(
            r#"
            SELECT *
            FROM devices
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let mut device_list = Vec::new();
        for row in rows {
            let created_at_str: String = row.get("created_at");
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)?.with_timezone(&Utc);
            let last_seen_str: String = row.get("last_seen");
            let last_seen = DateTime::parse_from_rfc3339(&last_seen_str)?.with_timezone(&Utc);
            
            let status_str: String = row.get("status");
            let status = match status_str.as_str() {
                "Online" => DeviceStatus::Online,
                "Offline" => DeviceStatus::Offline,
                "Error" => DeviceStatus::Error,
                "Updating" => DeviceStatus::Updating,
                "Maintenance" => DeviceStatus::Maintenance,
                _ => DeviceStatus::Offline,
            };

            let device = Device {
                mac_address: row.get("mac_address"),
                name: row.get("name"),
                alias: row.try_get::<Option<String>, _>("alias").unwrap_or(None),
                owner_id: row.get("owner_id"),
                ip_address: row.get("ip_address"),
                status,
                maintenance_mode: row.get("maintenance_mode"),
                firmware_version: row.get("firmware_version"),
                last_seen,
                created_at,
                connection_type: row.try_get("connection_type").unwrap_or_else(|_| "tcp".to_string()),
            };

            device_list.push(device);
        }

        Ok(device_list)
    }

    pub async fn update_device(
        &self,
        device_id: &str,
        name: Option<Option<&str>>,
        alias: Option<Option<&str>>,
        maintenance_mode: Option<bool>
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Handle name: Some(Some(value)) = set value, Some(None) = clear (not allowed for name), None = don't update
        if let Some(name_value) = name {
            sqlx::query("UPDATE devices SET name = ? WHERE mac_address = ?")
                .bind(name_value) // This binds Option<&str>
                .bind(device_id)
                .execute(&self.pool)
                .await?;
        }

        // Handle alias: Some(Some(value)) = set value, Some(None) = clear alias, None = don't update
        if let Some(alias_value) = alias {
            sqlx::query("UPDATE devices SET alias = ? WHERE mac_address = ?")
                .bind(alias_value) // This binds Option<&str> which can be None to clear the field
                .bind(device_id)
                .execute(&self.pool)
                .await?;
        }

        if let Some(maintenance_mode) = maintenance_mode {
            sqlx::query("UPDATE devices SET maintenance_mode = ? WHERE mac_address = ?")
                .bind(maintenance_mode)
                .bind(device_id)
                .execute(&self.pool)
                .await?;
        }

        Ok(())
    }

    pub async fn update_device_status(&self, device_id: &str, status: &DeviceStatus, ip_address: Option<&str>, firmware_version: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
        let status_str = match status {
            DeviceStatus::Online => "Online",
            DeviceStatus::Offline => "Offline",
            DeviceStatus::Error => "Error", 
            DeviceStatus::Updating => "Updating",
            DeviceStatus::Maintenance => "Maintenance",
        };
        
        let now = Utc::now().to_rfc3339();
        
        sqlx::query("UPDATE devices SET status = ?, ip_address = ?, firmware_version = ?, last_seen = ? WHERE mac_address = ?")
            .bind(status_str)
            .bind(ip_address)
            .bind(firmware_version)
            .bind(now)
            .bind(device_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn delete_device(&self, device_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Zuerst Berechtigungen löschen
        sqlx::query("DELETE FROM device_permissions WHERE device_id = ?")
            .bind(device_id)
            .execute(&self.pool)
            .await?;

        // Dann Device löschen
        sqlx::query("DELETE FROM devices WHERE mac_address = ?")
            .bind(device_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Get all Devices by connection type (e.g., "tcp" or "uart")
    pub async fn get_devices_by_connection_type(
        &self,
        connection_type: &str,
    ) -> Result<Vec<Device>, Box<dyn std::error::Error>> {
        let rows = sqlx::query(
            "SELECT * FROM devices WHERE connection_type = ? ORDER BY last_seen DESC"
        )
        .bind(connection_type)
        .fetch_all(&self.pool)
        .await?;

        let mut devices = Vec::new();
        for row in rows {
            let created_at_str: String = row.get("created_at");
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)?.with_timezone(&Utc);
            let last_seen_str: String = row.get("last_seen");
            let last_seen = DateTime::parse_from_rfc3339(&last_seen_str)?.with_timezone(&Utc);

            let status_str: String = row.get("status");
            let status = match status_str.as_str() {
                "Online" => DeviceStatus::Online,
                "Offline" => DeviceStatus::Offline,
                "Error" => DeviceStatus::Error,
                "Updating" => DeviceStatus::Updating,
                "Maintenance" => DeviceStatus::Maintenance,
                _ => DeviceStatus::Offline,
            };

            let device = Device {
                mac_address: row.get("mac_address"),
                name: row.get("name"),
                alias: row.try_get::<Option<String>, _>("alias").unwrap_or(None),
                owner_id: row.get("owner_id"),
                ip_address: row.get("ip_address"),
                status,
                maintenance_mode: row.get("maintenance_mode"),
                firmware_version: row.get("firmware_version"),
                last_seen,
                created_at,
                connection_type: row.try_get("connection_type").unwrap_or_else(|_| "tcp".to_string()),
            };

            devices.push(device);
        }

        Ok(devices)
    }

    /// Create or update device from discovery (auto-save discovered devices)
    /// If device exists, update IP and last_seen. If not, create as guest-owned device.
    pub async fn upsert_discovered_device(
        &self,
        mac_address: String,
        device_name: String,
        ip_address: Option<String>,
        connection_type: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Check if device already exists
        let existing = self.get_device_by_id(&mac_address).await?;

        if let Some(mut device) = existing {
            // Device exists - update IP and last_seen
            device.ip_address = ip_address;
            device.last_seen = Utc::now();

            sqlx::query(
                "UPDATE devices SET ip_address = ?, last_seen = ? WHERE mac_address = ?"
            )
            .bind(&device.ip_address)
            .bind(device.last_seen.to_rfc3339())
            .bind(&mac_address)
            .execute(&self.pool)
            .await?;

            tracing::debug!("Updated existing device in DB: {}", mac_address);
        } else {
            // Device doesn't exist - create new one with guest owner
            let new_device = Device {
                mac_address: mac_address.clone(),
                name: device_name,
                alias: None,
                owner_id: "guest".to_string(),
                ip_address,
                status: DeviceStatus::Offline,
                maintenance_mode: false,
                firmware_version: None,
                last_seen: Utc::now(),
                created_at: Utc::now(),
                connection_type: connection_type.unwrap_or_else(|| "tcp".to_string()),
            };

            self.create_device(new_device).await?;
            tracing::info!("Auto-saved new discovered device to DB: {}", mac_address);
        }

        Ok(())
    }

    // ============================================================================
    // DEVICE PERMISSIONS - Berechtigungsverwaltung
    // ============================================================================

    pub async fn set_device_permission(&self, device_id: &str, user_id: &str, permission: &str) -> Result<(), Box<dyn std::error::Error>> {
        sqlx::query(
            "INSERT OR REPLACE INTO device_permissions (device_id, user_id, permission) VALUES (?, ?, ?)"
        )
        .bind(device_id)
        .bind(user_id)
        .bind(permission)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn remove_device_permission(&self, device_id: &str, user_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        sqlx::query("DELETE FROM device_permissions WHERE device_id = ? AND user_id = ?")
            .bind(device_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn get_device_permissions(&self, device_id: &str) -> Result<Vec<DevicePermission>, Box<dyn std::error::Error>> {
        let rows = sqlx::query("SELECT * FROM device_permissions WHERE device_id = ?")
            .bind(device_id)
            .fetch_all(&self.pool)
            .await?;

        let mut permissions = Vec::new();
        for row in rows {
            permissions.push(DevicePermission {
                device_id: row.get("device_id"),
                user_id: row.get("user_id"),
                permission: row.get("permission"),
            });
        }

        Ok(permissions)
    }

    pub async fn get_user_device_permission(&self, device_id: &str, user_id: &str) -> Result<Option<String>, Box<dyn std::error::Error>> {
        let row = sqlx::query("SELECT permission FROM device_permissions WHERE device_id = ? AND user_id = ?")
            .bind(device_id)
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(row) => Ok(Some(row.get("permission"))),
            None => Ok(None),
        }
    }

    pub async fn user_has_device_permission(&self, device_id: &str, user_id: &str, required_permission: &str) -> Result<bool, Box<dyn std::error::Error>> {
        let user_permission = self.get_user_device_permission(device_id, user_id).await?;
        
        match user_permission {
            Some(permission) => {
                let has_permission = match required_permission {
                    "R" => ["R", "W", "V", "M", "O"].contains(&permission.as_str()),
                    "W" => {
                        // Prüfen ob Device im Wartungsmodus ist
                        let device = self.get_device_by_id(device_id).await?;
                        if let Some(device) = device {
                            if device.maintenance_mode {
                                ["V", "M", "O"].contains(&permission.as_str())
                            } else {
                                ["W", "V", "M", "O"].contains(&permission.as_str())
                            }
                        } else {
                            false
                        }
                    },
                    "V" => ["V", "M", "O"].contains(&permission.as_str()),
                    "M" => ["M", "O"].contains(&permission.as_str()),
                    "O" => permission == "O",
                    _ => false,
                };
                Ok(has_permission)
            }
            None => Ok(false),
        }
    }

    // ========================================================================
    // UART SETTINGS METHODS
    // ========================================================================

    /// Get UART settings from database
    pub async fn get_uart_settings(&self) -> Result<Option<(Option<String>, u32, bool)>, Box<dyn std::error::Error>> {
        let row = sqlx::query(
            "SELECT port, baud_rate, auto_connect FROM uart_settings WHERE id = 1"
        )
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let port: Option<String> = row.try_get("port")?;
                let baud_rate: i64 = row.try_get("baud_rate")?;
                let auto_connect: bool = row.try_get("auto_connect")?;
                Ok(Some((port, baud_rate as u32, auto_connect)))
            }
            None => Ok(None),
        }
    }

    /// Update UART settings in database
    pub async fn update_uart_settings(
        &self,
        port: Option<&str>,
        baud_rate: u32,
        auto_connect: bool
    ) -> Result<(), Box<dyn std::error::Error>> {
        sqlx::query(
            r#"
            UPDATE uart_settings
            SET port = ?, baud_rate = ?, auto_connect = ?, updated_at = datetime('now')
            WHERE id = 1
            "#
        )
        .bind(port)
        .bind(baud_rate as i64)
        .bind(auto_connect)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // ========================================================================
    // DEBUG SETTINGS METHODS
    // ========================================================================

    /// Get debug settings from database
    pub async fn get_debug_settings(&self) -> Result<Option<u32>, Box<dyn std::error::Error>> {
        let row = sqlx::query(
            "SELECT max_debug_messages FROM debug_settings WHERE id = 1"
        )
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let max_messages: i64 = row.try_get("max_debug_messages")?;
                Ok(Some(max_messages as u32))
            }
            None => Ok(None),
        }
    }

    /// Update debug settings in database
    pub async fn update_debug_settings(
        &self,
        max_debug_messages: u32
    ) -> Result<(), Box<dyn std::error::Error>> {
        sqlx::query(
            r#"
            UPDATE debug_settings
            SET max_debug_messages = ?, updated_at = datetime('now')
            WHERE id = 1
            "#
        )
        .bind(max_debug_messages as i64)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // HELPER FUNCTIONS FOR TESTS
    // ========================================================================

    /// Create a temporary in-memory test database
    async fn create_test_db() -> DatabaseManager {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let db = DatabaseManager { pool };
        db.init_database().await.unwrap();
        db
    }

    /// Create a test user with default values
    fn create_test_user(email: &str, password: &str) -> DatabaseUser {
        DatabaseUser::new(
            email.to_string(),
            "Test User".to_string(),
            password,
        ).unwrap()
    }

    /// Create a test device with default values
    fn create_test_device(mac: &str, owner_id: &str) -> Device {
        Device::new(
            "Test Device".to_string(),
            owner_id.to_string(),
            mac.to_string(),
        )
    }

    // ========================================================================
    // STRUCT TESTS - DatabaseUser
    // ========================================================================

    #[test]
    fn test_database_user_new() {
        let user = DatabaseUser::new(
            "test@example.com".to_string(),
            "Test User".to_string(),
            "password123",
        ).unwrap();

        assert_eq!(user.email, "test@example.com");
        assert_eq!(user.display_name, "Test User");
        assert_eq!(user.is_admin, false);
        assert!(!user.id.is_empty());
        assert!(!user.password_hash.is_empty());
        assert_ne!(user.password_hash, "password123");
    }

    #[test]
    fn test_database_user_verify_password_correct() {
        let user = create_test_user("test@example.com", "correct_password");
        let result = user.verify_password("correct_password").unwrap();
        assert!(result, "Correct password should verify successfully");
    }

    #[test]
    fn test_database_user_verify_password_incorrect() {
        let user = create_test_user("test@example.com", "correct_password");
        let result = user.verify_password("wrong_password").unwrap();
        assert!(!result, "Incorrect password should fail verification");
    }

    #[test]
    fn test_database_user_password_hash_not_plaintext() {
        let user = create_test_user("test@example.com", "mypassword");
        assert_ne!(user.password_hash, "mypassword");
        assert!(user.password_hash.starts_with("$2"));
    }

    // ========================================================================
    // STRUCT TESTS - Device
    // ========================================================================

    #[test]
    fn test_device_new_tcp() {
        let device = Device::new(
            "ESP32-001".to_string(),
            "user-123".to_string(),
            "AA:BB:CC:DD:EE:FF".to_string(),
        );

        assert_eq!(device.mac_address, "AA:BB:CC:DD:EE:FF");
        assert_eq!(device.name, "ESP32-001");
        assert_eq!(device.owner_id, "user-123");
        assert_eq!(device.connection_type, "tcp");
        assert_eq!(device.alias, None);
        assert!(matches!(device.status, DeviceStatus::Offline));
        assert_eq!(device.maintenance_mode, false);
    }

    #[test]
    fn test_device_new_uart() {
        let device = Device::new_uart(
            "ESP32-UART".to_string(),
            "user-456".to_string(),
            "11:22:33:44:55:66".to_string(),
        );

        assert_eq!(device.connection_type, "uart");
        assert_eq!(device.name, "ESP32-UART");
    }

    #[test]
    fn test_device_update_status() {
        let mut device = create_test_device("AA:BB:CC:DD:EE:FF", "user-1");
        let initial_time = device.last_seen;

        std::thread::sleep(std::time::Duration::from_millis(10));

        device.update_status(
            DeviceStatus::Online,
            Some("192.168.1.100".to_string()),
        );

        assert!(matches!(device.status, DeviceStatus::Online));
        assert_eq!(device.ip_address, Some("192.168.1.100".to_string()));
        assert!(device.last_seen > initial_time);
    }

    // ========================================================================
    // DATABASE TESTS - User Management
    // ========================================================================

    #[tokio::test]
    async fn test_create_and_get_user_by_email() {
        let db = create_test_db().await;
        let user = create_test_user("john@example.com", "password123");

        db.create_user(user.clone()).await.unwrap();

        let retrieved = db.get_user_by_email("john@example.com").await.unwrap();
        assert!(retrieved.is_some());
        let retrieved_user = retrieved.unwrap();
        assert_eq!(retrieved_user.email, "john@example.com");
        assert_eq!(retrieved_user.display_name, "Test User");
    }

    #[tokio::test]
    async fn test_get_user_by_email_not_found() {
        let db = create_test_db().await;
        let result = db.get_user_by_email("nonexistent@example.com").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_create_user_duplicate_email_fails() {
        let db = create_test_db().await;
        let user1 = create_test_user("duplicate@example.com", "pass1");
        let user2 = create_test_user("duplicate@example.com", "pass2");

        db.create_user(user1).await.unwrap();
        let result = db.create_user(user2).await;
        assert!(result.is_err(), "Duplicate email should fail");
    }

    #[tokio::test]
    async fn test_get_user_by_id() {
        let db = create_test_db().await;
        let user = create_test_user("test@example.com", "password");
        let user_id = user.id.clone();

        db.create_user(user).await.unwrap();

        let retrieved = db.get_user_by_id(&user_id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, user_id);
    }

    #[tokio::test]
    async fn test_update_user_display_name() {
        let db = create_test_db().await;
        let user = create_test_user("user@example.com", "password");
        let user_id = user.id.clone();

        db.create_user(user).await.unwrap();
        db.update_user_display_name(&user_id, "New Name").await.unwrap();

        let updated = db.get_user_by_id(&user_id).await.unwrap().unwrap();
        assert_eq!(updated.display_name, "New Name");
    }

    #[tokio::test]
    async fn test_get_all_users() {
        let db = create_test_db().await;

        let user1 = create_test_user("user1@example.com", "pass1");
        let user2 = create_test_user("user2@example.com", "pass2");

        db.create_user(user1).await.unwrap();
        db.create_user(user2).await.unwrap();

        let all_users = db.get_all_users().await.unwrap();
        // guest (system) + 2 created
        assert_eq!(all_users.len(), 3);
    }

    #[tokio::test]
    async fn test_search_users_by_email() {
        let db = create_test_db().await;

        let user1 = create_test_user("john.doe@example.com", "pass");
        let user2 = create_test_user("jane.smith@example.com", "pass");

        db.create_user(user1).await.unwrap();
        db.create_user(user2).await.unwrap();

        let results = db.search_users("john").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].email, "john.doe@example.com");
    }

    #[tokio::test]
    async fn test_search_users_by_display_name() {
        let db = create_test_db().await;

        let mut user = create_test_user("test@example.com", "pass");
        user.display_name = "Johnny Tester".to_string();

        db.create_user(user).await.unwrap();

        let results = db.search_users("Johnny").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].display_name, "Johnny Tester");
    }

    #[tokio::test]
    async fn test_get_users_paginated() {
        let db = create_test_db().await;

        for i in 1..=5 {
            let user = create_test_user(&format!("user{}@example.com", i), "pass");
            db.create_user(user).await.unwrap();
        }

        let page1 = db.get_users_paginated(0, 2).await.unwrap();
        assert_eq!(page1.len(), 2);

        let page2 = db.get_users_paginated(2, 2).await.unwrap();
        assert_eq!(page2.len(), 2);
    }

    #[tokio::test]
    async fn test_delete_user() {
        let db = create_test_db().await;
        let user = create_test_user("delete@example.com", "pass");
        let user_id = user.id.clone();

        db.create_user(user).await.unwrap();
        assert!(db.get_user_by_id(&user_id).await.unwrap().is_some());

        db.delete_user(&user_id).await.unwrap();
        assert!(db.get_user_by_id(&user_id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_update_user_admin_status() {
        let db = create_test_db().await;
        let user = create_test_user("admin@example.com", "pass");
        let user_id = user.id.clone();

        db.create_user(user).await.unwrap();

        let user_before = db.get_user_by_id(&user_id).await.unwrap().unwrap();
        assert_eq!(user_before.is_admin, false);

        db.update_user_admin_status(&user_id, true).await.unwrap();

        let user_after = db.get_user_by_id(&user_id).await.unwrap().unwrap();
        assert_eq!(user_after.is_admin, true);
    }

    #[tokio::test]
    async fn test_guest_user_exists_after_init() {
        let db = create_test_db().await;
        let guest = db.get_user_by_id("guest").await.unwrap();

        assert!(guest.is_some());
        let guest_user = guest.unwrap();
        assert_eq!(guest_user.id, "guest");
        assert_eq!(guest_user.email, "guest@system.local");
    }

    // ========================================================================
    // DATABASE TESTS - Device Management
    // ========================================================================

    #[tokio::test]
    async fn test_create_and_get_device() {
        let db = create_test_db().await;

        let owner = create_test_user("owner@example.com", "pass");
        let owner_id = owner.id.clone();
        db.create_user(owner).await.unwrap();

        let device = create_test_device("AA:BB:CC:DD:EE:FF", &owner_id);
        db.create_device(device).await.unwrap();

        let retrieved = db.get_device_by_id("AA:BB:CC:DD:EE:FF").await.unwrap();
        assert!(retrieved.is_some());
        let retrieved_device = retrieved.unwrap();
        assert_eq!(retrieved_device.mac_address, "AA:BB:CC:DD:EE:FF");
        assert_eq!(retrieved_device.name, "Test Device");
    }

    #[tokio::test]
    async fn test_create_device_with_guest_owner() {
        let db = create_test_db().await;
        let device = create_test_device("11:22:33:44:55:66", "guest");

        let result = db.create_device(device).await;
        assert!(result.is_ok(), "Device with guest owner should be created");
    }

    #[tokio::test]
    async fn test_list_user_devices() {
        let db = create_test_db().await;

        let user = create_test_user("user@example.com", "pass");
        let user_id = user.id.clone();
        db.create_user(user).await.unwrap();

        let device1 = create_test_device("AA:BB:CC:DD:EE:FF", &user_id);
        let device2 = create_test_device("11:22:33:44:55:66", &user_id);

        db.create_device(device1).await.unwrap();
        db.create_device(device2).await.unwrap();

        let devices = db.list_user_devices(&user_id).await.unwrap();
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].1, "O");
        assert_eq!(devices[1].1, "O");
    }

    #[tokio::test]
    async fn test_list_all_devices() {
        let db = create_test_db().await;

        let device1 = create_test_device("AA:BB:CC:DD:EE:FF", "guest");
        let device2 = create_test_device("11:22:33:44:55:66", "guest");

        db.create_device(device1).await.unwrap();
        db.create_device(device2).await.unwrap();

        let all_devices = db.list_all_devices().await.unwrap();
        assert_eq!(all_devices.len(), 2);
    }

    #[tokio::test]
    async fn test_update_device_name() {
        let db = create_test_db().await;
        let device = create_test_device("AA:BB:CC:DD:EE:FF", "guest");
        db.create_device(device).await.unwrap();

        db.update_device("AA:BB:CC:DD:EE:FF", Some(Some("New Device Name")), None, None)
            .await.unwrap();

        let updated = db.get_device_by_id("AA:BB:CC:DD:EE:FF").await.unwrap().unwrap();
        assert_eq!(updated.name, "New Device Name");
    }

    #[tokio::test]
    async fn test_update_device_alias() {
        let db = create_test_db().await;
        let device = create_test_device("AA:BB:CC:DD:EE:FF", "guest");
        db.create_device(device).await.unwrap();

        db.update_device("AA:BB:CC:DD:EE:FF", None, Some(Some("My Custom Alias")), None)
            .await.unwrap();

        let updated = db.get_device_by_id("AA:BB:CC:DD:EE:FF").await.unwrap().unwrap();
        assert_eq!(updated.alias, Some("My Custom Alias".to_string()));
    }

    #[tokio::test]
    async fn test_update_device_alias_clear() {
        let db = create_test_db().await;
        let mut device = create_test_device("AA:BB:CC:DD:EE:FF", "guest");
        device.alias = Some("Initial Alias".to_string());
        db.create_device(device).await.unwrap();

        db.update_device("AA:BB:CC:DD:EE:FF", None, Some(None), None)
            .await.unwrap();

        let updated = db.get_device_by_id("AA:BB:CC:DD:EE:FF").await.unwrap().unwrap();
        assert_eq!(updated.alias, None);
    }

    #[tokio::test]
    async fn test_update_device_maintenance_mode() {
        let db = create_test_db().await;
        let device = create_test_device("AA:BB:CC:DD:EE:FF", "guest");
        db.create_device(device).await.unwrap();

        db.update_device("AA:BB:CC:DD:EE:FF", None, None, Some(true))
            .await.unwrap();

        let updated = db.get_device_by_id("AA:BB:CC:DD:EE:FF").await.unwrap().unwrap();
        assert_eq!(updated.maintenance_mode, true);
    }

    #[tokio::test]
    async fn test_update_device_status() {
        let db = create_test_db().await;
        let device = create_test_device("AA:BB:CC:DD:EE:FF", "guest");
        db.create_device(device).await.unwrap();

        db.update_device_status(
            "AA:BB:CC:DD:EE:FF",
            &DeviceStatus::Online,
            Some("192.168.1.50"),
            Some("v1.2.3"),
        ).await.unwrap();

        let updated = db.get_device_by_id("AA:BB:CC:DD:EE:FF").await.unwrap().unwrap();
        assert!(matches!(updated.status, DeviceStatus::Online));
        assert_eq!(updated.ip_address, Some("192.168.1.50".to_string()));
        assert_eq!(updated.firmware_version, Some("v1.2.3".to_string()));
    }

    #[tokio::test]
    async fn test_delete_device() {
        let db = create_test_db().await;
        let device = create_test_device("AA:BB:CC:DD:EE:FF", "guest");
        db.create_device(device).await.unwrap();

        assert!(db.get_device_by_id("AA:BB:CC:DD:EE:FF").await.unwrap().is_some());

        db.delete_device("AA:BB:CC:DD:EE:FF").await.unwrap();

        assert!(db.get_device_by_id("AA:BB:CC:DD:EE:FF").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_get_devices_by_connection_type() {
        let db = create_test_db().await;

        let tcp_device = create_test_device("AA:BB:CC:DD:EE:FF", "guest");
        let uart_device = Device::new_uart(
            "UART Device".to_string(),
            "guest".to_string(),
            "11:22:33:44:55:66".to_string(),
        );

        db.create_device(tcp_device).await.unwrap();
        db.create_device(uart_device).await.unwrap();

        let tcp_devices = db.get_devices_by_connection_type("tcp").await.unwrap();
        assert_eq!(tcp_devices.len(), 1);
        assert_eq!(tcp_devices[0].connection_type, "tcp");

        let uart_devices = db.get_devices_by_connection_type("uart").await.unwrap();
        assert_eq!(uart_devices.len(), 1);
        assert_eq!(uart_devices[0].connection_type, "uart");
    }

    #[tokio::test]
    async fn test_upsert_discovered_device_new() {
        let db = create_test_db().await;

        db.upsert_discovered_device(
            "AA:BB:CC:DD:EE:FF".to_string(),
            "Discovered ESP32".to_string(),
            Some("192.168.1.100".to_string()),
            Some("tcp".to_string()),
        ).await.unwrap();

        let device = db.get_device_by_id("AA:BB:CC:DD:EE:FF").await.unwrap();
        assert!(device.is_some());
        let device = device.unwrap();
        assert_eq!(device.name, "Discovered ESP32");
        assert_eq!(device.owner_id, "guest");
        assert_eq!(device.ip_address, Some("192.168.1.100".to_string()));
    }

    #[tokio::test]
    async fn test_upsert_discovered_device_existing() {
        let db = create_test_db().await;

        let device = create_test_device("AA:BB:CC:DD:EE:FF", "guest");
        db.create_device(device).await.unwrap();

        db.upsert_discovered_device(
            "AA:BB:CC:DD:EE:FF".to_string(),
            "Updated Name".to_string(),
            Some("192.168.1.200".to_string()),
            Some("tcp".to_string()),
        ).await.unwrap();

        let device = db.get_device_by_id("AA:BB:CC:DD:EE:FF").await.unwrap().unwrap();
        assert_eq!(device.name, "Test Device"); // Name unchanged
        assert_eq!(device.ip_address, Some("192.168.1.200".to_string()));
    }

    // ========================================================================
    // DATABASE TESTS - Device Permissions
    // ========================================================================

    #[tokio::test]
    async fn test_set_and_get_device_permission() {
        let db = create_test_db().await;

        let user = create_test_user("user@example.com", "pass");
        let user_id = user.id.clone();
        db.create_user(user).await.unwrap();

        let device = create_test_device("AA:BB:CC:DD:EE:FF", &user_id);
        db.create_device(device).await.unwrap();

        let permission = db.get_user_device_permission("AA:BB:CC:DD:EE:FF", &user_id)
            .await.unwrap();
        assert_eq!(permission, Some("O".to_string()));
    }

    #[tokio::test]
    async fn test_set_device_permission_manual() {
        let db = create_test_db().await;

        let user1 = create_test_user("user1@example.com", "pass");
        let user2 = create_test_user("user2@example.com", "pass");
        let user1_id = user1.id.clone();
        let user2_id = user2.id.clone();

        db.create_user(user1).await.unwrap();
        db.create_user(user2).await.unwrap();

        let device = create_test_device("AA:BB:CC:DD:EE:FF", &user1_id);
        db.create_device(device).await.unwrap();

        db.set_device_permission("AA:BB:CC:DD:EE:FF", &user2_id, "R")
            .await.unwrap();

        let permission = db.get_user_device_permission("AA:BB:CC:DD:EE:FF", &user2_id)
            .await.unwrap();
        assert_eq!(permission, Some("R".to_string()));
    }

    #[tokio::test]
    async fn test_get_device_permissions() {
        let db = create_test_db().await;

        let user1 = create_test_user("user1@example.com", "pass");
        let user2 = create_test_user("user2@example.com", "pass");
        db.create_user(user1.clone()).await.unwrap();
        db.create_user(user2.clone()).await.unwrap();

        let device = create_test_device("AA:BB:CC:DD:EE:FF", &user1.id);
        db.create_device(device).await.unwrap();

        db.set_device_permission("AA:BB:CC:DD:EE:FF", &user2.id, "W")
            .await.unwrap();

        let permissions = db.get_device_permissions("AA:BB:CC:DD:EE:FF")
            .await.unwrap();
        assert_eq!(permissions.len(), 2);
    }

    #[tokio::test]
    async fn test_remove_device_permission() {
        let db = create_test_db().await;

        let user = create_test_user("user@example.com", "pass");
        db.create_user(user.clone()).await.unwrap();

        let device = create_test_device("AA:BB:CC:DD:EE:FF", &user.id);
        db.create_device(device).await.unwrap();

        assert!(db.get_user_device_permission("AA:BB:CC:DD:EE:FF", &user.id)
            .await.unwrap().is_some());

        db.remove_device_permission("AA:BB:CC:DD:EE:FF", &user.id)
            .await.unwrap();

        assert!(db.get_user_device_permission("AA:BB:CC:DD:EE:FF", &user.id)
            .await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_user_has_device_permission_read() {
        let db = create_test_db().await;

        let user = create_test_user("user@example.com", "pass");
        db.create_user(user.clone()).await.unwrap();

        let device = create_test_device("AA:BB:CC:DD:EE:FF", &user.id);
        db.create_device(device).await.unwrap();

        db.set_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "R")
            .await.unwrap();

        let has_read = db.user_has_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "R")
            .await.unwrap();
        assert!(has_read);

        let has_write = db.user_has_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "W")
            .await.unwrap();
        assert!(!has_write, "Read permission should not grant write");
    }

    #[tokio::test]
    async fn test_user_has_device_permission_hierarchy() {
        let db = create_test_db().await;

        let user = create_test_user("user@example.com", "pass");
        db.create_user(user.clone()).await.unwrap();

        let device = create_test_device("AA:BB:CC:DD:EE:FF", &user.id);
        db.create_device(device).await.unwrap();

        db.set_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "O")
            .await.unwrap();

        assert!(db.user_has_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "R").await.unwrap());
        assert!(db.user_has_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "W").await.unwrap());
        assert!(db.user_has_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "V").await.unwrap());
        assert!(db.user_has_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "M").await.unwrap());
        assert!(db.user_has_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "O").await.unwrap());
    }

    #[tokio::test]
    async fn test_user_has_device_permission_maintenance_mode() {
        let db = create_test_db().await;

        let user = create_test_user("user@example.com", "pass");
        db.create_user(user.clone()).await.unwrap();

        let device = create_test_device("AA:BB:CC:DD:EE:FF", &user.id);
        db.create_device(device).await.unwrap();

        db.set_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "W")
            .await.unwrap();

        let can_write = db.user_has_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "W")
            .await.unwrap();
        assert!(can_write);

        db.update_device("AA:BB:CC:DD:EE:FF", None, None, Some(true))
            .await.unwrap();

        let can_write_maintenance = db.user_has_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "W")
            .await.unwrap();
        assert!(!can_write_maintenance, "Write should be blocked in maintenance mode");
    }

    // ========================================================================
    // DATABASE TESTS - UART Settings
    // ========================================================================

    #[tokio::test]
    async fn test_get_uart_settings_default() {
        let db = create_test_db().await;
        let settings = db.get_uart_settings().await.unwrap();

        assert!(settings.is_some());
        let (port, baud_rate, auto_connect) = settings.unwrap();
        assert_eq!(port, None);
        assert_eq!(baud_rate, 115200);
        assert_eq!(auto_connect, false);
    }

    #[tokio::test]
    async fn test_update_uart_settings() {
        let db = create_test_db().await;

        db.update_uart_settings(Some("COM3"), 9600, true)
            .await.unwrap();

        let settings = db.get_uart_settings().await.unwrap().unwrap();
        assert_eq!(settings.0, Some("COM3".to_string()));
        assert_eq!(settings.1, 9600);
        assert_eq!(settings.2, true);
    }

    #[tokio::test]
    async fn test_update_uart_settings_clear_port() {
        let db = create_test_db().await;

        db.update_uart_settings(Some("COM3"), 115200, false)
            .await.unwrap();

        db.update_uart_settings(None, 115200, false)
            .await.unwrap();

        let settings = db.get_uart_settings().await.unwrap().unwrap();
        assert_eq!(settings.0, None);
    }

    // ========================================================================
    // DATABASE TESTS - Debug Settings
    // ========================================================================

    #[tokio::test]
    async fn test_get_debug_settings_default() {
        let db = create_test_db().await;
        let max_messages = db.get_debug_settings().await.unwrap();

        assert!(max_messages.is_some());
        assert_eq!(max_messages.unwrap(), 200);
    }

    #[tokio::test]
    async fn test_update_debug_settings() {
        let db = create_test_db().await;

        db.update_debug_settings(500).await.unwrap();

        let max_messages = db.get_debug_settings().await.unwrap().unwrap();
        assert_eq!(max_messages, 500);
    }

    // ========================================================================
    // EDGE CASE TESTS
    // ========================================================================

    #[tokio::test]
    async fn test_delete_user_cascades_permissions() {
        let db = create_test_db().await;

        let user = create_test_user("user@example.com", "pass");
        let user_id = user.id.clone();
        db.create_user(user).await.unwrap();

        let device = create_test_device("AA:BB:CC:DD:EE:FF", &user_id);
        db.create_device(device).await.unwrap();

        assert!(db.get_user_device_permission("AA:BB:CC:DD:EE:FF", &user_id)
            .await.unwrap().is_some());

        db.delete_user(&user_id).await.unwrap();

        let permissions = db.get_device_permissions("AA:BB:CC:DD:EE:FF")
            .await.unwrap();
        assert_eq!(permissions.len(), 0);
    }

    #[tokio::test]
    async fn test_delete_device_cascades_permissions() {
        let db = create_test_db().await;

        let user = create_test_user("user@example.com", "pass");
        db.create_user(user.clone()).await.unwrap();

        let device = create_test_device("AA:BB:CC:DD:EE:FF", &user.id);
        db.create_device(device).await.unwrap();

        db.delete_device("AA:BB:CC:DD:EE:FF").await.unwrap();

        let permissions = db.get_device_permissions("AA:BB:CC:DD:EE:FF")
            .await.unwrap();
        assert_eq!(permissions.len(), 0);
    }

    #[test]
    fn test_device_status_enum_serialization() {
        let status = DeviceStatus::Online;
        let status_str = match status {
            DeviceStatus::Online => "Online",
            DeviceStatus::Offline => "Offline",
            DeviceStatus::Error => "Error",
            DeviceStatus::Updating => "Updating",
            DeviceStatus::Maintenance => "Maintenance",
        };
        assert_eq!(status_str, "Online");
    }

    #[tokio::test]
    async fn test_foreign_key_constraint_invalid_owner() {
        let db = create_test_db().await;

        let device = create_test_device("AA:BB:CC:DD:EE:FF", "non-existent-user-id");
        let result = db.create_device(device).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_search_users_empty_query() {
        let db = create_test_db().await;
        let results = db.search_users("").await.unwrap();

        assert!(results.len() >= 1);
    }

    #[tokio::test]
    async fn test_pagination_beyond_results() {
        let db = create_test_db().await;

        let page = db.get_users_paginated(100, 10).await.unwrap();
        assert_eq!(page.len(), 0);
    }

    // ========================================================================
    // PERMISSION HIERARCHY TESTS - Alle Stufen einzeln prüfen
    // ========================================================================

    #[tokio::test]
    async fn test_permission_hierarchy_write() {
        let db = create_test_db().await;

        let user = create_test_user("user@example.com", "pass");
        db.create_user(user.clone()).await.unwrap();

        let device = create_test_device("AA:BB:CC:DD:EE:FF", &user.id);
        db.create_device(device).await.unwrap();

        db.set_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "W")
            .await.unwrap();

        assert!(db.user_has_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "R").await.unwrap(),
            "W should grant R");
        assert!(db.user_has_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "W").await.unwrap(),
            "W should grant W");
        assert!(!db.user_has_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "V").await.unwrap(),
            "W should NOT grant V");
        assert!(!db.user_has_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "M").await.unwrap(),
            "W should NOT grant M");
        assert!(!db.user_has_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "O").await.unwrap(),
            "W should NOT grant O");
    }

    #[tokio::test]
    async fn test_permission_hierarchy_view() {
        let db = create_test_db().await;

        let user = create_test_user("user@example.com", "pass");
        db.create_user(user.clone()).await.unwrap();

        let device = create_test_device("AA:BB:CC:DD:EE:FF", &user.id);
        db.create_device(device).await.unwrap();

        db.set_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "V")
            .await.unwrap();

        assert!(db.user_has_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "R").await.unwrap(),
            "V should grant R");
        assert!(db.user_has_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "W").await.unwrap(),
            "V should grant W");
        assert!(db.user_has_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "V").await.unwrap(),
            "V should grant V");
        assert!(!db.user_has_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "M").await.unwrap(),
            "V should NOT grant M");
        assert!(!db.user_has_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "O").await.unwrap(),
            "V should NOT grant O");
    }

    #[tokio::test]
    async fn test_permission_hierarchy_manage() {
        let db = create_test_db().await;

        let user = create_test_user("user@example.com", "pass");
        db.create_user(user.clone()).await.unwrap();

        let device = create_test_device("AA:BB:CC:DD:EE:FF", &user.id);
        db.create_device(device).await.unwrap();

        db.set_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "M")
            .await.unwrap();

        assert!(db.user_has_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "R").await.unwrap(),
            "M should grant R");
        assert!(db.user_has_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "W").await.unwrap(),
            "M should grant W");
        assert!(db.user_has_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "V").await.unwrap(),
            "M should grant V");
        assert!(db.user_has_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "M").await.unwrap(),
            "M should grant M");
        assert!(!db.user_has_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "O").await.unwrap(),
            "M should NOT grant O");
    }

    #[tokio::test]
    async fn test_maintenance_mode_higher_permissions_can_write() {
        let db = create_test_db().await;

        let user_v = create_test_user("view@example.com", "pass");
        let user_m = create_test_user("manage@example.com", "pass");
        let user_o = create_test_user("owner@example.com", "pass");
        db.create_user(user_v.clone()).await.unwrap();
        db.create_user(user_m.clone()).await.unwrap();
        db.create_user(user_o.clone()).await.unwrap();

        let device = create_test_device("AA:BB:CC:DD:EE:FF", &user_o.id);
        db.create_device(device).await.unwrap();

        db.set_device_permission("AA:BB:CC:DD:EE:FF", &user_v.id, "V").await.unwrap();
        db.set_device_permission("AA:BB:CC:DD:EE:FF", &user_m.id, "M").await.unwrap();

        // Maintenance-Mode aktivieren
        db.update_device("AA:BB:CC:DD:EE:FF", None, None, Some(true)).await.unwrap();

        // V, M, O können trotz Maintenance-Mode schreiben
        assert!(db.user_has_device_permission("AA:BB:CC:DD:EE:FF", &user_v.id, "W").await.unwrap(),
            "V should still write in maintenance mode");
        assert!(db.user_has_device_permission("AA:BB:CC:DD:EE:FF", &user_m.id, "W").await.unwrap(),
            "M should still write in maintenance mode");
        assert!(db.user_has_device_permission("AA:BB:CC:DD:EE:FF", &user_o.id, "W").await.unwrap(),
            "O should still write in maintenance mode");
    }

    #[tokio::test]
    async fn test_user_has_no_permission() {
        let db = create_test_db().await;

        let owner = create_test_user("owner@example.com", "pass");
        let other = create_test_user("other@example.com", "pass");
        db.create_user(owner.clone()).await.unwrap();
        db.create_user(other.clone()).await.unwrap();

        let device = create_test_device("AA:BB:CC:DD:EE:FF", &owner.id);
        db.create_device(device).await.unwrap();

        // other hat keine Permission für das Device
        assert!(!db.user_has_device_permission("AA:BB:CC:DD:EE:FF", &other.id, "R").await.unwrap(),
            "User without any permission should not have R");
        assert!(!db.user_has_device_permission("AA:BB:CC:DD:EE:FF", &other.id, "W").await.unwrap(),
            "User without any permission should not have W");
        assert!(!db.user_has_device_permission("AA:BB:CC:DD:EE:FF", &other.id, "O").await.unwrap(),
            "User without any permission should not have O");
    }

    #[tokio::test]
    async fn test_permission_upgrade_overwrites() {
        let db = create_test_db().await;

        let user = create_test_user("user@example.com", "pass");
        db.create_user(user.clone()).await.unwrap();

        let device = create_test_device("AA:BB:CC:DD:EE:FF", &user.id);
        db.create_device(device).await.unwrap();

        // Setze R
        db.set_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "R").await.unwrap();
        assert!(!db.user_has_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "W").await.unwrap(),
            "R should not grant W");

        // Upgrade auf W
        db.set_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "W").await.unwrap();
        assert!(db.user_has_device_permission("AA:BB:CC:DD:EE:FF", &user.id, "W").await.unwrap(),
            "After upgrade to W, should have W");

        // Nur eine Permission-Zeile sollte existieren
        let perms = db.get_device_permissions("AA:BB:CC:DD:EE:FF").await.unwrap();
        let user_perms: Vec<_> = perms.iter().filter(|p| p.user_id == user.id).collect();
        assert_eq!(user_perms.len(), 1, "Should have exactly one permission entry after upgrade");
        assert_eq!(user_perms[0].permission, "W");
    }

    #[tokio::test]
    async fn test_get_device_by_id_not_found() {
        let db = create_test_db().await;

        let result = db.get_device_by_id("non-existent-mac").await.unwrap();
        assert!(result.is_none(), "Non-existent device should return None");
    }
}
