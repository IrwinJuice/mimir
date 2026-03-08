# Mimir Analytics

> Mímir — у скандинавській міфології постать, що символізує мудрість та пораду.

**Mimir Analytics** — це open-source, self-hosted інструмент для локального аналізу банківських транзакцій. Підключайте свої банківські рахунки, синхронізуйте транзакції та досліджуйте витрати за допомогою графіків і потужних фільтрів — усе на вашому комп'ютері, дані ніколи не покидають його.


![readme_title.png](assets/readme_title.png)


> ⚠️ Не відкривайте порт (за замовчуванням `42000`) у публічний інтернет ⚠️

---

## Можливості

- 📥 **Підключення до банків** — Підтримує інтеграцію з банками (наразі підтримується тільки [Monobank](https://monobank.ua/)) через відкритий API для автоматичного завантаження транзакцій.
- 💾 **Локальне збереження (SQLite)** — Всі дані зберігаються в локальному файлі `mimir.db`. Ніяких хмар, ніяких третіх сторін.
- 📊 **Графіки та аналітика** — Візуалізація витрат у часі, за категоріями або за рахунками.
- 🔍 **Розширені фільтри** — Фільтрація транзакцій за діапазоном дат, рахунком, сумою, валютою, категорією MCC, описом тощо. Комбінуйте фільтри за допомогою `AND`, `OR`, `AND NOT`, `OR NOT`.
- 📤 **Експорт** — Завантажуйте відфільтровані транзакції у форматах **CSV**, **XLSX** або **JSON**.
---

## Завантаження та запуск

Попередньо зібрані бінарні файли для **Windows** та **Linux** доступні на [сторінці релізів](https://github.com/IrwinJuice/mimir/releases).

### Windows

1. Завантажте `mimir-windows-x86_64` з [останніх релізів](https://github.com/IrwinJuice/mimir/releases/latest).
2. Розпакуйте архів.
3. Відредагуйте `Mimir.toml`, щоб встановити бажаний порт (за замовчуванням `42000`).
4. Запустіть виконуваний файл:
   ```powershell
   .\mimir.exe
   ```
5. Відкрийте `http://localhost:42000` у браузері.

### Linux

1. Завантажте `mimir-linux-x86_64` з [останніх релізів](https://github.com/IrwinJuice/mimir/releases/latest).
2. Розпакуйте архів:
   ```bash
   tar -xzf mimir-linux-x86_64.tar.gz
   cd mimir
   ```
3. Відредагуйте `Mimir.toml`, щоб встановити бажаний порт (за замовчуванням `42000`).
4. Зробіть бінарний файл виконуваним і запустіть його:
   ```bash
   chmod +x mimir
   ./mimir
   ```
5. Відкрийте `http://localhost:42000` у браузері.


## Технології

| Рівень    | Технологія                          |
|-----------|-------------------------------------|
| Бекенд    | Rust, [Axum](https://github.com/tokio-rs/axum), Tokio |
| База даних| SQLite через [SQLx](https://github.com/launchbadge/sqlx) |
| Фронтенд  | Angular (статичні файли обслуговуються з `./static`) |

---

## Для розробників

### Передумови

> Фронтенд: https://github.com/IrwinJuice/mimir-client
 
- [Rust](https://www.rust-lang.org/tools/install) (edition 2024)
- Токен API Monobank ([отримується у додатку Monobank](https://api.monobank.ua/))

### Збірка та запуск

```bash
# Клонуйте репозиторій
git clone https://github.com/your-username/mimir.git
cd mimir

# Збірка у режимі release
cargo build --release

# Запуск
./target/release/mimir
```

Сервер за замовчуванням запускається на `http://localhost:42000`.  
Відкрийте `http://localhost:42000` у браузері, щоб отримати доступ до інтерфейсу.

### Конфігурація

Відредагуйте `Mimir.toml`, щоб змінити порт:

```toml
[service]
port = 42000
```

---

## Структура проекту

```
src/
├── main.rs               # Налаштування сервера, маршрути
├── config.rs             # Завантаження конфігурації (Mimir.toml)
├── mcc_data.rs           # Пошук по MCC кодах
├── ws_handler.rs         # WebSocket для реального часу
├── account/
│   ├── handler.rs        # HTTP-обробники для рахунків
│   ├── model.rs          # Моделі даних рахунків
│   └── repository.rs     # DB-запити для рахунків
├── transaction/
│   ├── handler.rs        # HTTP-обробники транзакцій (запит, експорт)
│   ├── model.rs          # Моделі транзакцій і структури фільтрів
│   └── repository.rs     # DB-запити транзакцій з динамічними фільтрами
└── utils/
    └── datetime.rs       # Допоміжні функції для дати/часу
```

---

### Запуск у режимі розробки

**Бекенд** (гаряча перезбірка з `cargo-watch`):

```bash
cargo install cargo-watch
cargo watch -x run
```

Бекенд слухає на `http://localhost:42000`.

### База даних

Міграції керуються SQLx і запускаються автоматично при старті з каталогу `migrations/`.

Файли міграцій мають формат `NNN_description.sql`.

### Логування та трасування

Рівень логування керується змінною оточення `RUST_LOG`:

```bash
# Windows (PowerShell)
$env:RUST_LOG = "debug"
cargo run

# Linux / macOS
RUST_LOG=debug cargo run
```
---

## Участь у проекті

Внески вітаються! Дотримуйтесь наведених нижче вказівок, щоб історія проекту залишалася чистою та послідовною.

### Повідомлення комітів

Проект використовує [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/).  
Кожне повідомлення коміту повинно відповідати формату:

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

**Типи:**

| Type | Коли використовувати |
|------|----------------------|
| `feat` | Нова можливість |
| `fix` | Виправлення помилки |
| `docs` | Зміни у документації |
| `refactor` | Зміни у коді без додавання фіч або виправлення багів |
| `perf` | Поліпшення продуктивності |
| `test` | Додавання або оновлення тестів |
| `chore` | Оновлення побудови, залежностей або інструментів |
| `ci` | Зміни у CI/CD конфігурації |

**Приклади:**

```
feat(transaction): add XLSX export support
fix(account): handle missing token gracefully
docs: add contribution guide
chore: update sqlx to 0.8
```

Значні зміни повинні бути помічені `!` після типу/сфери або мати `BREAKING CHANGE:` у футері:

```
feat(api)!: rename /bills endpoint to /transactions
```

### Гілки

- `main` — стабільний, готовий до продакшну код
- `feat/<short-description>` — нові можливості
- `fix/<short-description>` — виправлення помилок

### Pull Requests

1. Форкніть репозиторій та створіть гілку від `main`.
2. Переконайтеся, що проєкт збирається без помилок: `cargo build`.
3. Запустіть `cargo clippy` і виправте попередження.
4. Відформатуйте код за допомогою `cargo fmt`.
5. Відкрийте Pull Request з чітким описом того, що змінено і чому.

---

## Ліцензія

Див. [LICENSE.md](LICENSE.md).

