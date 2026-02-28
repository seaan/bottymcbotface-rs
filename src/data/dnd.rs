use chrono::{Datelike, Duration, NaiveDate, Utc};
use sqlx::{FromRow, Pool, Sqlite};

const DND_SCHEMA_SQL: &str = include_str!("../../data/db/migrations/202602280001_dnd_month_planner.sql");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonthKey {
    January,
    February,
    March,
    April,
    May,
    June,
    July,
    August,
    September,
    October,
    November,
    December,
}

impl MonthKey {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "jan" | "january" => Ok(Self::January),
            "feb" | "february" => Ok(Self::February),
            "mar" | "march" => Ok(Self::March),
            "apr" | "april" => Ok(Self::April),
            "may" => Ok(Self::May),
            "jun" | "june" => Ok(Self::June),
            "jul" | "july" => Ok(Self::July),
            "aug" | "august" => Ok(Self::August),
            "sep" | "sept" | "september" => Ok(Self::September),
            "oct" | "october" => Ok(Self::October),
            "nov" | "november" => Ok(Self::November),
            "dec" | "december" => Ok(Self::December),
            _ => Err("invalid month name".to_string()),
        }
    }

    pub fn number(self) -> u32 {
        match self {
            Self::January => 1,
            Self::February => 2,
            Self::March => 3,
            Self::April => 4,
            Self::May => 5,
            Self::June => 6,
            Self::July => 7,
            Self::August => 8,
            Self::September => 9,
            Self::October => 10,
            Self::November => 11,
            Self::December => 12,
        }
    }

    pub fn canonical_name(self) -> &'static str {
        match self {
            Self::January => "january",
            Self::February => "february",
            Self::March => "march",
            Self::April => "april",
            Self::May => "may",
            Self::June => "june",
            Self::July => "july",
            Self::August => "august",
            Self::September => "september",
            Self::October => "october",
            Self::November => "november",
            Self::December => "december",
        }
    }
}

pub fn is_read_only(month: MonthKey, now: NaiveDate) -> bool {
    month.number() < now.month()
}

#[derive(Clone)]
pub struct DndRepository {
    pool: Pool<Sqlite>,
}

#[derive(FromRow, Debug, Clone)]
pub struct DndMonth {
    pub id: i64,
    pub guild_id: String,
    pub month: String,
    pub access_token: String,
}

#[derive(FromRow, Debug, Clone)]
pub struct DndParticipant {
    pub id: i64,
    pub month_id: i64,
    pub name: String,
    pub edit_token: String,
}

impl DndRepository {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }

    pub async fn init_for_tests(&self) -> Result<(), sqlx::Error> {
        sqlx::raw_sql(DND_SCHEMA_SQL).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn ensure_month(&self, guild_id: &str, month: MonthKey) -> Result<DndMonth, sqlx::Error> {
        let now = Utc::now().timestamp();
        let token = uuid::Uuid::new_v4().to_string();

        sqlx::query(
            "INSERT INTO dnd_months (guild_id, month, title, access_token, created_at)
             VALUES (?, ?, NULL, ?, ?)
             ON CONFLICT(guild_id, month) DO NOTHING",
        )
        .bind(guild_id)
        .bind(month.canonical_name())
        .bind(token)
        .bind(now)
        .execute(&self.pool)
        .await?;

        sqlx::query_as::<_, DndMonth>(
            "SELECT id, guild_id, month, access_token
             FROM dnd_months
             WHERE guild_id = ? AND month = ?
             LIMIT 1",
        )
        .bind(guild_id)
        .bind(month.canonical_name())
        .fetch_one(&self.pool)
        .await
    }

    pub async fn ensure_participant(&self, month_id: i64, name: &str) -> Result<DndParticipant, sqlx::Error> {
        let now = Utc::now().timestamp();

        sqlx::query(
            "INSERT INTO dnd_participants (month_id, name, edit_token, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(month_id, name) DO NOTHING",
        )
        .bind(month_id)
        .bind(name)
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        sqlx::query_as::<_, DndParticipant>(
            "SELECT id, month_id, name, edit_token
             FROM dnd_participants
             WHERE month_id = ? AND name = ?
             LIMIT 1",
        )
        .bind(month_id)
        .bind(name)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn issue_login_code(
        &self,
        participant_id: i64,
        ttl: Duration,
    ) -> Result<String, sqlx::Error> {
        let code = uuid::Uuid::new_v4().simple().to_string()[0..8].to_ascii_uppercase();
        let now = Utc::now().timestamp();
        let expires_at = (Utc::now() + ttl).timestamp();

        sqlx::query(
            "INSERT INTO dnd_login_codes (participant_id, code, expires_at, created_at, used_count)
             VALUES (?, ?, ?, ?, 0)",
        )
        .bind(participant_id)
        .bind(&code)
        .bind(expires_at)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(code)
    }

    pub async fn validate_login_code(
        &self,
        participant_id: i64,
        code: &str,
    ) -> Result<bool, sqlx::Error> {
        let now = Utc::now().timestamp();
        let exists = sqlx::query_as::<_, (i64,)>(
            "SELECT 1
             FROM dnd_login_codes
             WHERE participant_id = ? AND code = ? AND expires_at >= ?
             LIMIT 1",
        )
        .bind(participant_id)
        .bind(code)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;

        if exists.is_some() {
            sqlx::query(
                "UPDATE dnd_login_codes
                 SET used_count = used_count + 1
                 WHERE participant_id = ? AND code = ?",
            )
            .bind(participant_id)
            .bind(code)
            .execute(&self.pool)
            .await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::{is_read_only, DndRepository, MonthKey};

    #[test]
    fn month_parses_case_insensitive() {
        assert_eq!(MonthKey::parse("January").unwrap(), MonthKey::January);
        assert_eq!(MonthKey::parse("sep").unwrap(), MonthKey::September);
    }

    #[test]
    fn month_rejects_invalid_value() {
        assert!(MonthKey::parse("foo").is_err());
    }

    #[test]
    fn month_readonly_cutoff_works() {
        let now = NaiveDate::from_ymd_opt(2026, 4, 1).unwrap();
        assert!(is_read_only(MonthKey::March, now));
        assert!(!is_read_only(MonthKey::April, now));
        assert!(!is_read_only(MonthKey::December, now));
    }

    async fn test_repo() -> DndRepository {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        let repo = DndRepository::new(pool);
        repo.init_for_tests().await.unwrap();
        repo
    }

    async fn seed_participant(repo: &DndRepository) -> super::DndParticipant {
        let month = repo.ensure_month("123", MonthKey::March).await.unwrap();
        repo.ensure_participant(month.id, "sean").await.unwrap()
    }

    #[tokio::test]
    async fn ensure_month_creates_once_per_guild() {
        let repo = test_repo().await;
        let first = repo.ensure_month("123", MonthKey::March).await.unwrap();
        let second = repo.ensure_month("123", MonthKey::March).await.unwrap();
        assert_eq!(first.id, second.id);
    }

    #[tokio::test]
    async fn ensure_participant_creates_once_per_name() {
        let repo = test_repo().await;
        let month = repo.ensure_month("123", MonthKey::March).await.unwrap();
        let first = repo.ensure_participant(month.id, "sean").await.unwrap();
        let second = repo.ensure_participant(month.id, "sean").await.unwrap();
        assert_eq!(first.id, second.id);
    }

    #[tokio::test]
    async fn code_is_valid_multiple_times_before_expiry() {
        let repo = test_repo().await;
        let participant = seed_participant(&repo).await;
        let code = repo
            .issue_login_code(participant.id, chrono::Duration::minutes(15))
            .await
            .unwrap();

        assert!(repo
            .validate_login_code(participant.id, &code)
            .await
            .unwrap());
        assert!(repo
            .validate_login_code(participant.id, &code)
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn expired_code_is_rejected() {
        let repo = test_repo().await;
        let participant = seed_participant(&repo).await;
        let code = repo
            .issue_login_code(participant.id, chrono::Duration::seconds(-1))
            .await
            .unwrap();

        assert!(!repo
            .validate_login_code(participant.id, &code)
            .await
            .unwrap());
    }
}
