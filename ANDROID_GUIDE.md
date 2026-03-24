# Complete Android App Specification: Zen - Topic-Based Spaced Repetition

## 1. App Concept & Purpose

### What is Zen?

Zen is a **topic-based spaced repetition learning app** that helps users memorize and understand any subject using:
1. **LLM-generated questions** (fresh every time)
2. **LLM-evaluated answers** (automatic grading with feedback)
3. **FSRS algorithm** (optimal review scheduling)

### Key Innovation

Unlike traditional flashcard apps (Anki, Quizlet), Zen uses **LLMs to generate unique questions** on each review, testing understanding rather than rote memorization.

### Example Use Case

**User wants to learn "LSTM neural networks":**
1. User adds topic: `"LSTM, recurrent neural networks, vanishing gradient"`
2. App schedules first review in 24 hours
3. At review time:
   - LLM generates Question 1: "How does LSTM solve the vanishing gradient problem?"
   - User types answer (multi-line)
   - LLM grades answer: 75/100 + feedback
   - Repeat for Questions 2 and 3
   - Average score (say 80%) → Rating "Good" → Next review in 5 days
4. FSRS algorithm adjusts future intervals based on performance

---

## 2. Complete User Flow

### 2.1 First Launch

```
┌─────────────────────────────────────┐
│     Welcome to Zen                  │
│                                     │
│  [Configure LLM API]                │
│                                     │
│  Need: Groq API key                 │
│  (Free at groq.com)                 │
│                                     │
│  [ Enter API Key ]                  │
│  [ Continue ]                       │
└─────────────────────────────────────┘
```

### 2.2 Home Screen (Empty State)

```
┌─────────────────────────────────────┐
│  Zen                    [Stats] [⚙] │
├─────────────────────────────────────┤
│                                     │
│     No topics yet!                  │
│                                     │
│     Tap + to add your               │
│     first learning topic            │
│                                     │
│                                     │
│                          [+]        │
└─────────────────────────────────────┘
```

### 2.3 Adding a Topic

```
┌─────────────────────────────────────┐
│  [←] Add Topic                      │
├─────────────────────────────────────┤
│                                     │
│  Enter keywords (comma-separated):  │
│  ┌─────────────────────────────────┐│
│  │ LSTM, recurrent neural networks,││
│  │ vanishing gradient, memory cells││
│  └─────────────────────────────────┘│
│                                     │
│  💡 Tips:                           │
│  • Use 1-5 related keywords         │
│  • Be specific (not "AI", but       │
│    "LSTM architecture")             │
│  • Group related concepts           │
│                                     │
│            [Add Topic]              │
└─────────────────────────────────────┘
```

**What happens:**
- Topic saved to database with unique ID
- Initial schedule: due in 24 hours
- User returns to Home

### 2.4 Home Screen (With Topics)

```
┌─────────────────────────────────────┐
│  Zen                    [Stats] [⚙] │
├─────────────────────────────────────┤
│  ┌───────────────────────────────┐ │
│  │  📚 3 topics due for review   │ │
│  │                               │ │
│  │     [Start Review Session]    │ │
│  └───────────────────────────────┘ │
│                                     │
│  All Topics:                        │
│  ┌───────────────────────────────┐ │
│  │ LSTM, RNN, vanishing gradient │ │
│  │ Due: Today                    │ │
│  │ Reviews: 0                    │ │
│  └───────────────────────────────┘ │
│  ┌───────────────────────────────┐ │
│  │ React hooks, useState         │ │
│  │ Due: Tomorrow                 │ │
│  │ Reviews: 5                    │ │
│  └───────────────────────────────┘ │
│                          [+]        │
└─────────────────────────────────────┘
```

### 2.5 Review Session - Question 1

```
┌─────────────────────────────────────┐
│  [←] Review                         │
├─────────────────────────────────────┤
│  Topic 1 of 3                       │
│  LSTM, RNN, vanishing gradient      │
├─────────────────────────────────────┤
│  Question 1 of 3                    │
│  ┌─────────────────────────────────┐│
│  │ How does an LSTM architecture   ││
│  │ address the vanishing gradient  ││
│  │ problem that standard RNNs face?││
│  │                                 ││
│  │ Explain the role of gates.      ││
│  └─────────────────────────────────┘│
│                                     │
│  Your Answer:                       │
│  ┌─────────────────────────────────┐│
│  │ LSTMs use forget, input, and    ││
│  │ output gates to control         ││
│  │ information flow...█            ││
│  │                                 ││
│  └─────────────────────────────────┘│
│                                     │
│         [Submit Answer]             │
│                                     │
│  Rating Scale:                      │
│  90%+ → Easy | 70-90% → Good        │
│  60-70% → Hard | <60% → Again       │
└─────────────────────────────────────┘
```

**What happens:**
1. LLM generates question based on keywords
2. User types multi-line answer
3. User submits

### 2.6 Review Session - Feedback

```
┌─────────────────────────────────────┐
│  [←] Review                         │
├─────────────────────────────────────┤
│  Question 1 of 3                    │
│  ┌─────────────────────────────────┐│
│  │ How does an LSTM...             ││
│  └─────────────────────────────────┘│
│                                     │
│  Your Answer:                       │
│  ┌─────────────────────────────────┐│
│  │ LSTMs use forget, input, and    ││
│  │ output gates to control...      ││
│  └─────────────────────────────────┘│
│                                     │
│  ┌─────────────────────────────────┐│
│  │ ⭐ Score: 75/100 (Good)          ││
│  │                                 ││
│  │ Feedback: You correctly         ││
│  │ identified the three gates and  ││
│  │ their role. Could have mentioned││
│  │ the memory cell state (C_t) and ││
│  │ how it maintains long-term      ││
│  │ dependencies.                   ││
│  └─────────────────────────────────┘│
│                                     │
│        [Next Question]              │
└─────────────────────────────────────┘
```

**Repeat for Questions 2 and 3**

### 2.7 Review Session - Results

```
┌─────────────────────────────────────┐
│  [←] Review Complete                │
├─────────────────────────────────────┤
│  Topic 1 of 3                       │
│  LSTM, RNN, vanishing gradient      │
├─────────────────────────────────────┤
│  Results:                           │
│  • Question 1: 75/100 (Good)        │
│  • Question 2: 82/100 (Good)        │
│  • Question 3: 91/100 (Easy)        │
│                                     │
│  ─────────────────────────────      │
│  Average: 82.7%                     │
│  Final Rating: Good (3/4)           │
│  ─────────────────────────────      │
│                                     │
│  Next review: 5 days                │
│                                     │
│  ┌─────────────────────────────────┐│
│  │ Great work! You demonstrated    ││
│  │ solid understanding of LSTM     ││
│  │ concepts.                       ││
│  └─────────────────────────────────┘│
│                                     │
│         [Next Topic (2/3)]          │
└─────────────────────────────────────┘
```

**What happens:**
1. Average score calculated: (75 + 82 + 91) / 3 = 82.7%
2. Convert to rating: 82.7% → Good (3)
3. FSRS calculates next interval: ~5 days
4. Review data saved to database
5. Continue to next topic or finish session

### 2.8 Statistics Screen

```
┌─────────────────────────────────────┐
│  [←] Statistics                     │
├─────────────────────────────────────┤
│  Overview                           │
│  ┌─────────────────────────────────┐│
│  │ Total Topics: 15                ││
│  │ Due Today: 3                    ││
│  │ Due This Week: 8                ││
│  │ Reviews Completed: 47           ││
│  │ Average Score: 78.5%            ││
│  └─────────────────────────────────┘│
│                                     │
│  Recent Reviews                     │
│  ┌─────────────────────────────────┐│
│  │ LSTM, RNN... | 82.7% | Today    ││
│  │ React hooks... | 91% | Yesterday││
│  └─────────────────────────────────┘│
│                                     │
│  Performance Trend                  │
│  ┌─────────────────────────────────┐│
│  │      •─•─•                      ││
│  │    •       •─•                  ││
│  │  •             •                ││
│  └─────────────────────────────────┘│
└─────────────────────────────────────┘
```

---

## 3. Technical Specifications

### 3.1 Score-to-Rating Algorithm

**Conversion table (MUST be exact):**
```kotlin
fun scoreToRating(score: Double): Int {
    return when {
        score >= 90.0 -> 4  // Easy
        score >= 70.0 -> 3  // Good
        score >= 60.0 -> 2  // Hard
        else -> 1           // Again
    }
}
```

### 3.2 FSRS Algorithm (Complete Implementation)

```kotlin
class FSRSScheduler {
    companion object {
        const val DESIRED_RETENTION = 0.9
        const val MAX_INTERVAL_DAYS = 365.0
    }

    data class MemoryState(
        val stability: Double,
        val difficulty: Double
    )

    data class ReviewResult(
        val intervalDays: Double,
        val newStability: Double,
        val newDifficulty: Double
    )

    /**
     * Calculate next review interval using FSRS algorithm
     *
     * @param rating User rating (1=Again, 2=Hard, 3=Good, 4=Easy)
     * @param currentState Current memory state (null for new topics)
     * @param elapsedDays Days since last review (0 for new topics)
     * @return Next review schedule
     */
    fun calculateNextReview(
        rating: Int,
        currentState: MemoryState?,
        elapsedDays: Int
    ): ReviewResult {
        // Default for new topics
        val state = currentState ?: MemoryState(
            stability = 1.0,
            difficulty = 5.0
        )

        // Calculate retrievability (R)
        val retrievability = if (elapsedDays > 0) {
            Math.pow(Math.E, -elapsedDays.toDouble() / state.stability)
        } else {
            1.0
        }

        // Calculate new difficulty (D)
        // D = D - w6 * (rating - 3)
        // w6 = 0.5 (difficulty adjustment factor)
        val newDifficulty = (state.difficulty - 0.5 * (rating - 3))
            .coerceIn(1.0, 10.0)

        // Calculate new stability (S)
        val newStability = when (rating) {
            1 -> {
                // Again: S' = S * w11
                // w11 = 0.5 (failure stability multiplier)
                state.stability * 0.5
            }
            2 -> {
                // Hard: S' = S * (1 + exp(w8) * (11 - D) * Math.pow(S, -w9) * (exp((1 - R) * w10) - 1))
                // Simplified: S' = S * 0.9
                state.stability * 0.9
            }
            3 -> {
                // Good: S' = S * (1 + exp(w8) * (11 - D) * Math.pow(S, -w9) * (exp((1 - R) * w10) - 1))
                // Simplified for 90% retention: S' = S * 2.5
                state.stability * 2.5
            }
            4 -> {
                // Easy: bonus multiplier
                state.stability * 3.0
            }
            else -> state.stability
        }

        // Calculate interval
        // I = S * (ln(R) / ln(0.9))
        val interval = (newStability * (Math.log(DESIRED_RETENTION) / Math.log(0.9)))
            .coerceIn(0.5, MAX_INTERVAL_DAYS)

        return ReviewResult(
            intervalDays = interval,
            newStability = newStability,
            newDifficulty = newDifficulty
        )
    }
}
```

**FSRS Examples:**
- New topic, rated "Good (3)": Next review in ~2.5 days
- After 5 reviews, stability=10, rated "Easy (4)": Next review in ~30 days
- Rated "Again (1)": Next review in ~0.5 days (12 hours)

### 3.3 LLM Integration (Complete)

**Question Generation Prompt:**
```kotlin
fun buildQuestionPrompt(keywords: List<String>): String {
    return """
You are a teacher creating exam questions. Generate ONE clear, specific question that covers these topics: ${keywords.joinToString(", ")}

The question should:
- Test understanding of these concepts
- Be answerable in 2-3 sentences
- Be specific and focused
- Not be a yes/no question
- Require explanation or analysis

Provide ONLY the question text, nothing else. No preamble, no "Here's a question:".
    """.trimIndent()
}
```

**Example LLM Request:**
```json
{
  "model": "llama-3.3-70b-versatile",
  "messages": [
    {
      "role": "user",
      "content": "You are a teacher creating exam questions. Generate ONE clear, specific question that covers these topics: LSTM, recurrent neural networks, vanishing gradient\n\nThe question should:\n- Test understanding of these concepts\n- Be answerable in 2-3 sentences\n- Be specific and focused\n- Not be a yes/no question\n- Require explanation or analysis\n\nProvide ONLY the question text, nothing else. No preamble, no \"Here's a question:\"."
    }
  ],
  "temperature": 0.8,
  "max_tokens": 256
}
```

**Example LLM Response:**
```
How does an LSTM architecture address the vanishing gradient problem that standard RNNs face, and what role do the forget, input, and output gates play in maintaining long-term dependencies?
```

**Answer Evaluation Prompt:**
```kotlin
fun buildEvaluationPrompt(question: String, answer: String): String {
    return """
You are a strict teacher grading an exam answer. Be honest and critical.

Question: $question

Student's Answer: $answer

Evaluate the answer and provide:
SCORE: [0-100, where:]
  - 90-100: Excellent, complete understanding
  - 70-89: Good, mostly correct with minor gaps
  - 60-69: Partial understanding, missing key points
  - 40-59: Significant gaps, some correct elements
  - 0-39: Incorrect or severely incomplete

FEEDBACK: [One or two sentences explaining what was good or what was missing]

Be strict but fair. Partial answers should score in the 40-70 range.
    """.trimIndent()
}
```

**Example Evaluation Response:**
```
SCORE: 75
FEEDBACK: You correctly identified the three types of gates and their role in controlling information flow. However, you didn't explain how this specifically solves the vanishing gradient problem (the constant error carousel), and you missed mentioning the cell state that maintains long-term memory.
```

**Parsing Evaluation:**
```kotlin
data class ParsedEvaluation(val score: Double, val feedback: String)

fun parseEvaluation(content: String): ParsedEvaluation {
    var score = 0.0
    val feedbackLines = mutableListOf<String>()
    var inFeedback = false

    content.lines().forEach { line ->
        val trimmed = line.trim()
        when {
            trimmed.startsWith("SCORE:", ignoreCase = true) -> {
                // Extract number from "SCORE: 75" or "SCORE:75"
                val scoreText = trimmed.substringAfter(":", "")
                    .trim()
                    .takeWhile { it.isDigit() || it == '.' }
                score = scoreText.toDoubleOrNull() ?: 0.0
            }
            trimmed.startsWith("FEEDBACK:", ignoreCase = true) -> {
                inFeedback = true
                val feedbackStart = trimmed.substringAfter(":", "").trim()
                if (feedbackStart.isNotEmpty()) {
                    feedbackLines.add(feedbackStart)
                }
            }
            inFeedback && trimmed.isNotEmpty() -> {
                feedbackLines.add(trimmed)
            }
        }
    }

    val feedback = feedbackLines.joinToString(" ").ifBlank {
        "No feedback provided."
    }

    return ParsedEvaluation(score, feedback)
}
```

### 3.4 Database Schema (Complete)

**SQL Schema:**
```sql
-- Topics table
CREATE TABLE topics (
    id TEXT PRIMARY KEY,
    created_at INTEGER NOT NULL,
    modified_at INTEGER NOT NULL
);

-- Keywords (many-to-one with topics)
CREATE TABLE topic_keywords (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    topic_id TEXT NOT NULL,
    keyword TEXT NOT NULL,
    position INTEGER NOT NULL,
    FOREIGN KEY(topic_id) REFERENCES topics(id) ON DELETE CASCADE
);
CREATE INDEX idx_topic_keywords_topic_id ON topic_keywords(topic_id);

-- Scheduling data (one-to-one with topics)
CREATE TABLE topic_schedule (
    topic_id TEXT PRIMARY KEY,
    due_date INTEGER NOT NULL,
    stability REAL,
    difficulty REAL,
    last_review INTEGER,
    FOREIGN KEY(topic_id) REFERENCES topics(id) ON DELETE CASCADE
);

-- Review history
CREATE TABLE review_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    topic_id TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    rating INTEGER NOT NULL,
    scheduled_days REAL NOT NULL,
    elapsed_days REAL NOT NULL,
    average_score REAL NOT NULL,
    FOREIGN KEY(topic_id) REFERENCES topics(id) ON DELETE CASCADE
);

-- Individual question logs (for analytics)
CREATE TABLE question_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    review_log_id INTEGER NOT NULL,
    question_number INTEGER NOT NULL,
    generated_question TEXT NOT NULL,
    user_answer TEXT NOT NULL,
    llm_score REAL NOT NULL,
    llm_feedback TEXT,
    FOREIGN KEY(review_log_id) REFERENCES review_logs(id) ON DELETE CASCADE
);
```

**Entity Relationships:**
```
topics (1) ─── (many) topic_keywords
topics (1) ─── (1) topic_schedule
topics (1) ─── (many) review_logs
review_logs (1) ─── (many) question_logs
```

---

## 4. Complete Android Implementation

### 4.1 Project Setup

**build.gradle.kts (Project level):**
```kotlin
plugins {
    id("com.android.application") version "8.7.3" apply false
    id("org.jetbrains.kotlin.android") version "2.0.21" apply false
    id("com.google.devtools.ksp") version "2.0.21-1.0.29" apply false
    id("com.google.dagger.hilt.android") version "2.52" apply false
}
```

**build.gradle.kts (app level):**
```kotlin
plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("com.google.devtools.ksp")
    id("com.google.dagger.hilt.android")
}

android {
    namespace = "com.example.zen"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.example.zen"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "1.0.0"
    }

    buildFeatures {
        compose = true
    }

    composeOptions {
        kotlinCompilerExtensionVersion = "1.5.14"
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

dependencies {
    val composeBom = platform("androidx.compose:compose-bom:2024.10.01")
    implementation(composeBom)

    // Compose
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.activity:activity-compose:1.9.3")
    implementation("androidx.navigation:navigation-compose:2.8.4")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.8.7")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.8.7")

    // Room
    implementation("androidx.room:room-runtime:2.6.1")
    implementation("androidx.room:room-ktx:2.6.1")
    ksp("androidx.room:room-compiler:2.6.1")

    // Retrofit
    implementation("com.squareup.retrofit2:retrofit:2.11.0")
    implementation("com.squareup.retrofit2:converter-gson:2.11.0")
    implementation("com.squareup.okhttp3:logging-interceptor:4.12.0")

    // Hilt
    implementation("com.google.dagger:hilt-android:2.52")
    ksp("com.google.dagger:hilt-compiler:2.52")
    implementation("androidx.hilt:hilt-navigation-compose:1.2.0")

    // DataStore
    implementation("androidx.datastore:datastore-preferences:1.1.1")

    // Coroutines
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.9.0")
}
```

### 4.2 Data Layer - Complete Implementation

**ZenDatabase.kt:**
```kotlin
@Database(
    entities = [
        TopicEntity::class,
        TopicKeywordEntity::class,
        TopicScheduleEntity::class,
        ReviewLogEntity::class,
        QuestionLogEntity::class
    ],
    version = 1
)
abstract class ZenDatabase : RoomDatabase() {
    abstract fun topicDao(): TopicDao
    abstract fun scheduleDao(): ScheduleDao
    abstract fun reviewDao(): ReviewDao
}
```

**TopicEntity.kt:**
```kotlin
@Entity(tableName = "topics")
data class TopicEntity(
    @PrimaryKey val id: String,
    @ColumnInfo(name = "created_at") val createdAt: Long,
    @ColumnInfo(name = "modified_at") val modifiedAt: Long
)
```

**TopicKeywordEntity.kt:**
```kotlin
@Entity(
    tableName = "topic_keywords",
    foreignKeys = [
        ForeignKey(
            entity = TopicEntity::class,
            parentColumns = ["id"],
            childColumns = ["topic_id"],
            onDelete = ForeignKey.CASCADE
        )
    ],
    indices = [Index("topic_id")]
)
data class TopicKeywordEntity(
    @PrimaryKey(autoGenerate = true) val id: Int = 0,
    @ColumnInfo(name = "topic_id") val topicId: String,
    val keyword: String,
    val position: Int
)
```

**TopicScheduleEntity.kt:**
```kotlin
@Entity(
    tableName = "topic_schedule",
    foreignKeys = [
        ForeignKey(
            entity = TopicEntity::class,
            parentColumns = ["id"],
            childColumns = ["topic_id"],
            onDelete = ForeignKey.CASCADE
        )
    ]
)
data class TopicScheduleEntity(
    @PrimaryKey @ColumnInfo(name = "topic_id") val topicId: String,
    @ColumnInfo(name = "due_date") val dueDate: Long,
    val stability: Double?,
    val difficulty: Double?,
    @ColumnInfo(name = "last_review") val lastReview: Long?
)
```

**ReviewLogEntity.kt:**
```kotlin
@Entity(
    tableName = "review_logs",
    foreignKeys = [
        ForeignKey(
            entity = TopicEntity::class,
            parentColumns = ["id"],
            childColumns = ["topic_id"],
            onDelete = ForeignKey.CASCADE
        )
    ]
)
data class ReviewLogEntity(
    @PrimaryKey(autoGenerate = true) val id: Int = 0,
    @ColumnInfo(name = "topic_id") val topicId: String,
    val timestamp: Long,
    val rating: Int,
    @ColumnInfo(name = "scheduled_days") val scheduledDays: Double,
    @ColumnInfo(name = "elapsed_days") val elapsedDays: Double,
    @ColumnInfo(name = "average_score") val averageScore: Double
)
```

**QuestionLogEntity.kt:**
```kotlin
@Entity(
    tableName = "question_logs",
    foreignKeys = [
        ForeignKey(
            entity = ReviewLogEntity::class,
            parentColumns = ["id"],
            childColumns = ["review_log_id"],
            onDelete = ForeignKey.CASCADE
        )
    ]
)
data class QuestionLogEntity(
    @PrimaryKey(autoGenerate = true) val id: Int = 0,
    @ColumnInfo(name = "review_log_id") val reviewLogId: Int,
    @ColumnInfo(name = "question_number") val questionNumber: Int,
    @ColumnInfo(name = "generated_question") val generatedQuestion: String,
    @ColumnInfo(name = "user_answer") val userAnswer: String,
    @ColumnInfo(name = "llm_score") val llmScore: Double,
    @ColumnInfo(name = "llm_feedback") val llmFeedback: String?
)
```

**TopicDao.kt:**
```kotlin
@Dao
interface TopicDao {
    @Transaction
    suspend fun insertTopicWithKeywords(
        topic: TopicEntity,
        keywords: List<TopicKeywordEntity>
    ) {
        insertTopic(topic)
        insertKeywords(keywords)
    }

    @Insert
    suspend fun insertTopic(topic: TopicEntity)

    @Insert
    suspend fun insertKeywords(keywords: List<TopicKeywordEntity>)

    @Query("SELECT * FROM topics ORDER BY created_at DESC")
    fun getAllTopics(): Flow<List<TopicEntity>>

    @Transaction
    @Query("""
        SELECT t.*, ts.due_date
        FROM topics t
        LEFT JOIN topic_schedule ts ON t.id = ts.topic_id
        ORDER BY t.created_at DESC
    """)
    fun getAllTopicsWithSchedule(): Flow<List<TopicWithSchedule>>

    @Query("SELECT * FROM topic_keywords WHERE topic_id = :topicId ORDER BY position")
    suspend fun getKeywordsForTopic(topicId: String): List<TopicKeywordEntity>

    @Query("DELETE FROM topics WHERE id = :topicId")
    suspend fun deleteTopic(topicId: String)
}

data class TopicWithSchedule(
    @Embedded val topic: TopicEntity,
    @ColumnInfo(name = "due_date") val dueDate: Long?
)
```

**ScheduleDao.kt:**
```kotlin
@Dao
interface ScheduleDao {
    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertOrUpdateSchedule(schedule: TopicScheduleEntity)

    @Query("SELECT * FROM topic_schedule WHERE topic_id = :topicId")
    suspend fun getSchedule(topicId: String): TopicScheduleEntity?

    @Query("""
        SELECT t.*, ts.due_date, ts.stability, ts.difficulty, ts.last_review
        FROM topics t
        INNER JOIN topic_schedule ts ON t.id = ts.topic_id
        WHERE ts.due_date <= :now
        ORDER BY ts.due_date ASC
    """)
    suspend fun getDueTopics(now: Long): List<TopicWithScheduleDetails>

    @Query("SELECT COUNT(*) FROM topic_schedule WHERE due_date <= :now")
    suspend fun getDueCount(now: Long): Int
}

data class TopicWithScheduleDetails(
    @Embedded val topic: TopicEntity,
    @ColumnInfo(name = "due_date") val dueDate: Long,
    val stability: Double?,
    val difficulty: Double?,
    @ColumnInfo(name = "last_review") val lastReview: Long?
)
```

**ReviewDao.kt:**
```kotlin
@Dao
interface ReviewDao {
    @Transaction
    suspend fun insertReviewWithQuestions(
        reviewLog: ReviewLogEntity,
        questionLogs: List<QuestionLogEntity>
    ): Long {
        val reviewId = insertReviewLog(reviewLog)
        insertQuestionLogs(questionLogs.map { it.copy(reviewLogId = reviewId.toInt()) })
        return reviewId
    }

    @Insert
    suspend fun insertReviewLog(log: ReviewLogEntity): Long

    @Insert
    suspend fun insertQuestionLogs(logs: List<QuestionLogEntity>)

    @Query("SELECT COUNT(*) FROM review_logs")
    suspend fun getTotalReviewCount(): Int

    @Query("SELECT AVG(average_score) FROM review_logs")
    suspend fun getAverageScore(): Double?

    @Query("""
        SELECT * FROM review_logs
        WHERE topic_id = :topicId
        ORDER BY timestamp DESC
    """)
    suspend fun getReviewsForTopic(topicId: String): List<ReviewLogEntity>
}
```

### 4.3 Network Layer - Complete Implementation

**GroqApi.kt:**
```kotlin
interface GroqApi {
    @POST("openai/v1/chat/completions")
    suspend fun chatCompletion(
        @Body request: ChatRequest
    ): Response<ChatResponse>
}

data class ChatRequest(
    val model: String,
    val messages: List<ChatMessage>,
    val temperature: Double,
    @SerializedName("max_tokens") val maxTokens: Int
)

data class ChatMessage(
    val role: String,  // "user" or "assistant"
    val content: String
)

data class ChatResponse(
    val choices: List<ChatChoice>,
    val usage: ChatUsage?
)

data class ChatChoice(
    val message: ChatMessage
)

data class ChatUsage(
    @SerializedName("total_tokens") val totalTokens: Int
)
```

**NetworkModule.kt:**
```kotlin
@Module
@InstallIn(SingletonComponent::class)
object NetworkModule {

    @Provides
    @Singleton
    fun provideOkHttpClient(): OkHttpClient {
        return OkHttpClient.Builder()
            .addInterceptor { chain ->
                val apiKey = // Get from DataStore
                val request = chain.request().newBuilder()
                    .addHeader("Authorization", "Bearer $apiKey")
                    .addHeader("Content-Type", "application/json")
                    .build()
                chain.proceed(request)
            }
            .addInterceptor(HttpLoggingInterceptor().apply {
                level = HttpLoggingInterceptor.Level.BODY
            })
            .connectTimeout(30, TimeUnit.SECONDS)
            .readTimeout(30, TimeUnit.SECONDS)
            .build()
    }

    @Provides
    @Singleton
    fun provideRetrofit(okHttpClient: OkHttpClient): Retrofit {
        return Retrofit.Builder()
            .baseUrl("https://api.groq.com/")
            .client(okHttpClient)
            .addConverterFactory(GsonConverterFactory.create())
            .build()
    }

    @Provides
    @Singleton
    fun provideGroqApi(retrofit: Retrofit): GroqApi {
        return retrofit.create(GroqApi::class.java)
    }
}
```

**LLMRepository.kt:**
```kotlin
class LLMRepository @Inject constructor(
    private val api: GroqApi,
    private val settingsDataStore: SettingsDataStore
) {
    suspend fun generateQuestion(keywords: List<String>): Result<String> {
        return try {
            val model = settingsDataStore.getModel() ?: "llama-3.3-70b-versatile"
            val prompt = buildQuestionPrompt(keywords)

            val response = api.chatCompletion(
                ChatRequest(
                    model = model,
                    messages = listOf(ChatMessage("user", prompt)),
                    temperature = 0.8,
                    maxTokens = 256
                )
            )

            if (response.isSuccessful && response.body() != null) {
                val content = response.body()!!.choices.firstOrNull()?.message?.content
                if (content != null) {
                    Result.success(content.trim())
                } else {
                    Result.failure(Exception("No content in response"))
                }
            } else {
                Result.failure(Exception("API error: ${response.code()}"))
            }
        } catch (e: Exception) {
            Result.failure(e)
        }
    }

    suspend fun evaluateAnswer(question: String, answer: String): Result<ParsedEvaluation> {
        return try {
            val model = settingsDataStore.getModel() ?: "llama-3.3-70b-versatile"
            val prompt = buildEvaluationPrompt(question, answer)

            val response = api.chatCompletion(
                ChatRequest(
                    model = model,
                    messages = listOf(ChatMessage("user", prompt)),
                    temperature = 0.5,
                    maxTokens = 512
                )
            )

            if (response.isSuccessful && response.body() != null) {
                val content = response.body()!!.choices.firstOrNull()?.message?.content
                if (content != null) {
                    Result.success(parseEvaluation(content))
                } else {
                    Result.failure(Exception("No content in response"))
                }
            } else {
                Result.failure(Exception("API error: ${response.code()}"))
            }
        } catch (e: Exception) {
            Result.failure(e)
        }
    }

    private fun buildQuestionPrompt(keywords: List<String>): String {
        // Use the prompt from section 3.3
    }

    private fun buildEvaluationPrompt(question: String, answer: String): String {
        // Use the prompt from section 3.3
    }

    private fun parseEvaluation(content: String): ParsedEvaluation {
        // Use the parsing logic from section 3.3
    }
}

data class ParsedEvaluation(
    val score: Double,
    val feedback: String
)
```

### 4.4 Domain Layer - Complete Implementation

**Domain Models:**
```kotlin
data class Topic(
    val id: String,
    val keywords: List<String>,
    val createdAt: Long,
    val dueDate: Long?,
    val reviewCount: Int
)

data class ReviewSession(
    val topicId: String,
    val keywords: List<String>,
    val currentQuestionIndex: Int,
    val questions: List<QuestionData>
) {
    fun isComplete() = currentQuestionIndex >= 3

    fun averageScore(): Double {
        val scores = questions.mapNotNull { it.evaluation?.score }
        return if (scores.isNotEmpty()) scores.average() else 0.0
    }

    fun finalRating(): Int = scoreToRating(averageScore())
}

data class QuestionData(
    val question: String,
    val answer: String? = null,
    val evaluation: AnswerEvaluation? = null
)

data class AnswerEvaluation(
    val score: Double,
    val feedback: String
)

fun scoreToRating(score: Double): Int {
    return when {
        score >= 90.0 -> 4
        score >= 70.0 -> 3
        score >= 60.0 -> 2
        else -> 1
    }
}
```

**Use Cases:**

```kotlin
class AddTopicUseCase @Inject constructor(
    private val topicDao: TopicDao,
    private val scheduleDao: ScheduleDao
) {
    suspend operator fun invoke(keywordsText: String): Result<Unit> {
        return try {
            val keywords = keywordsText.split(",")
                .map { it.trim() }
                .filter { it.isNotEmpty() }

            if (keywords.isEmpty()) {
                return Result.failure(Exception("No keywords provided"))
            }

            if (keywords.size > 20) {
                return Result.failure(Exception("Maximum 20 keywords allowed"))
            }

            val topicId = UUID.randomUUID().toString().take(8)
            val now = System.currentTimeMillis()

            val topic = TopicEntity(
                id = topicId,
                createdAt = now,
                modifiedAt = now
            )

            val keywordEntities = keywords.mapIndexed { index, keyword ->
                TopicKeywordEntity(
                    topicId = topicId,
                    keyword = keyword,
                    position = index
                )
            }

            topicDao.insertTopicWithKeywords(topic, keywordEntities)

            // Schedule first review in 24 hours
            val dueDate = now + 24 * 60 * 60 * 1000
            scheduleDao.insertOrUpdateSchedule(
                TopicScheduleEntity(
                    topicId = topicId,
                    dueDate = dueDate,
                    stability = null,
                    difficulty = null,
                    lastReview = null
                )
            )

            Result.success(Unit)
        } catch (e: Exception) {
            Result.failure(e)
        }
    }
}

class GetDueTopicsUseCase @Inject constructor(
    private val scheduleDao: ScheduleDao,
    private val topicDao: TopicDao
) {
    suspend operator fun invoke(): List<Topic> {
        val now = System.currentTimeMillis()
        val dueTopicsWithSchedule = scheduleDao.getDueTopics(now)

        return dueTopicsWithSchedule.map { topicSchedule ->
            val keywords = topicDao.getKeywordsForTopic(topicSchedule.topic.id)
            Topic(
                id = topicSchedule.topic.id,
                keywords = keywords.map { it.keyword },
                createdAt = topicSchedule.topic.createdAt,
                dueDate = topicSchedule.dueDate,
                reviewCount = 0 // Can calculate from review_logs if needed
            )
        }
    }
}

class SubmitReviewUseCase @Inject constructor(
    private val reviewDao: ReviewDao,
    private val scheduleDao: ScheduleDao,
    private val fsrsScheduler: FSRSScheduler
) {
    suspend operator fun invoke(session: ReviewSession): Result<Unit> {
        return try {
            val averageScore = session.averageScore()
            val rating = session.finalRating()

            // Get current schedule
            val currentSchedule = scheduleDao.getSchedule(session.topicId)
            val memoryState = currentSchedule?.let {
                if (it.stability != null && it.difficulty != null) {
                    FSRSScheduler.MemoryState(it.stability, it.difficulty)
                } else null
            }

            val elapsedDays = currentSchedule?.lastReview?.let {
                ((System.currentTimeMillis() - it) / (24 * 60 * 60 * 1000)).toInt()
            } ?: 0

            // Calculate next review
            val nextReview = fsrsScheduler.calculateNextReview(
                rating = rating,
                currentState = memoryState,
                elapsedDays = elapsedDays
            )

            val now = System.currentTimeMillis()
            val nextDueDate = now + (nextReview.intervalDays * 24 * 60 * 60 * 1000).toLong()

            // Save review log
            val reviewLog = ReviewLogEntity(
                topicId = session.topicId,
                timestamp = now,
                rating = rating,
                scheduledDays = nextReview.intervalDays,
                elapsedDays = elapsedDays.toDouble(),
                averageScore = averageScore
            )

            val questionLogs = session.questions.mapIndexed { index, q ->
                QuestionLogEntity(
                    reviewLogId = 0, // Will be set by transaction
                    questionNumber = index + 1,
                    generatedQuestion = q.question,
                    userAnswer = q.answer ?: "",
                    llmScore = q.evaluation?.score ?: 0.0,
                    llmFeedback = q.evaluation?.feedback
                )
            }

            reviewDao.insertReviewWithQuestions(reviewLog, questionLogs)

            // Update schedule
            scheduleDao.insertOrUpdateSchedule(
                TopicScheduleEntity(
                    topicId = session.topicId,
                    dueDate = nextDueDate,
                    stability = nextReview.newStability,
                    difficulty = nextReview.newDifficulty,
                    lastReview = now
                )
            )

            Result.success(Unit)
        } catch (e: Exception) {
            Result.failure(e)
        }
    }
}
```

### 4.5 Presentation Layer - Review Screen (Complete)

**ReviewViewModel.kt:**
```kotlin
@HiltViewModel
class ReviewViewModel @Inject constructor(
    private val getDueTopicsUseCase: GetDueTopicsUseCase,
    private val llmRepository: LLMRepository,
    private val submitReviewUseCase: SubmitReviewUseCase
) : ViewModel() {

    private val _state = MutableStateFlow<ReviewScreenState>(ReviewScreenState.Loading)
    val state: StateFlow<ReviewScreenState> = _state.asStateFlow()

    private var allDueTopics: List<Topic> = emptyList()
    private var currentTopicIndex = 0
    private var currentSession: ReviewSession? = null

    init {
        loadDueTopics()
    }

    private fun loadDueTopics() {
        viewModelScope.launch {
            _state.value = ReviewScreenState.Loading

            val topics = getDueTopicsUseCase()
            allDueTopics = topics

            if (topics.isEmpty()) {
                _state.value = ReviewScreenState.NoTopics
            } else {
                startTopicReview(topics[0])
            }
        }
    }

    private suspend fun startTopicReview(topic: Topic) {
        _state.value = ReviewScreenState.Generating

        // Generate first question
        when (val result = llmRepository.generateQuestion(topic.keywords)) {
            is Result.Success -> {
                val session = ReviewSession(
                    topicId = topic.id,
                    keywords = topic.keywords,
                    currentQuestionIndex = 0,
                    questions = listOf(
                        QuestionData(question = result.data)
                    )
                )
                currentSession = session
                _state.value = ReviewScreenState.Answering(session, 0)
            }
            is Result.Failure -> {
                _state.value = ReviewScreenState.Error(result.exception.message ?: "Failed to generate question")
            }
        }
    }

    fun submitAnswer(answer: String) {
        val session = currentSession ?: return
        val questionIndex = session.currentQuestionIndex

        // Update question with answer
        val updatedQuestions = session.questions.toMutableList()
        updatedQuestions[questionIndex] = updatedQuestions[questionIndex].copy(answer = answer)
        currentSession = session.copy(questions = updatedQuestions)

        // Evaluate
        viewModelScope.launch {
            _state.value = ReviewScreenState.Evaluating

            val question = updatedQuestions[questionIndex].question
            when (val result = llmRepository.evaluateAnswer(question, answer)) {
                is Result.Success -> {
                    // Update question with evaluation
                    updatedQuestions[questionIndex] = updatedQuestions[questionIndex].copy(
                        evaluation = AnswerEvaluation(
                            score = result.data.score,
                            feedback = result.data.feedback
                        )
                    )
                    currentSession = session.copy(questions = updatedQuestions)
                    _state.value = ReviewScreenState.ShowingFeedback(
                        currentSession!!,
                        questionIndex
                    )
                }
                is Result.Failure -> {
                    _state.value = ReviewScreenState.Error(result.exception.message ?: "Failed to evaluate")
                }
            }
        }
    }

    fun continueToNextQuestion() {
        val session = currentSession ?: return
        val nextIndex = session.currentQuestionIndex + 1

        if (nextIndex >= 3) {
            // All questions done, show results
            _state.value = ReviewScreenState.Results(
                session = session,
                hasMoreTopics = currentTopicIndex < allDueTopics.size - 1
            )
        } else {
            // Generate next question
            viewModelScope.launch {
                _state.value = ReviewScreenState.Generating

                when (val result = llmRepository.generateQuestion(session.keywords)) {
                    is Result.Success -> {
                        val updatedQuestions = session.questions + QuestionData(result.data)
                        currentSession = session.copy(
                            questions = updatedQuestions,
                            currentQuestionIndex = nextIndex
                        )
                        _state.value = ReviewScreenState.Answering(currentSession!!, nextIndex)
                    }
                    is Result.Failure -> {
                        _state.value = ReviewScreenState.Error(result.exception.message ?: "Failed")
                    }
                }
            }
        }
    }

    fun submitAndContinue() {
        val session = currentSession ?: return

        viewModelScope.launch {
            _state.value = ReviewScreenState.Loading

            when (submitReviewUseCase(session)) {
                is Result.Success -> {
                    currentTopicIndex++
                    if (currentTopicIndex < allDueTopics.size) {
                        startTopicReview(allDueTopics[currentTopicIndex])
                    } else {
                        _state.value = ReviewScreenState.Complete
                    }
                }
                is Result.Failure -> {
                    _state.value = ReviewScreenState.Error("Failed to save review")
                }
            }
        }
    }
}

sealed class ReviewScreenState {
    object Loading : ReviewScreenState()
    object NoTopics : ReviewScreenState()
    object Generating : ReviewScreenState()
    data class Answering(val session: ReviewSession, val questionIndex: Int) : ReviewScreenState()
    object Evaluating : ReviewScreenState()
    data class ShowingFeedback(val session: ReviewSession, val questionIndex: Int) : ReviewScreenState()
    data class Results(val session: ReviewSession, val hasMoreTopics: Boolean) : ReviewScreenState()
    object Complete : ReviewScreenState()
    data class Error(val message: String) : ReviewScreenState()
}
```

**ReviewScreen.kt:**
```kotlin
@Composable
fun ReviewScreen(
    viewModel: ReviewViewModel = hiltViewModel(),
    onComplete: () -> Unit
) {
    val state by viewModel.state.collectAsStateWithLifecycle()

    when (val currentState = state) {
        is ReviewScreenState.Loading,
        is ReviewScreenState.Generating,
        is ReviewScreenState.Evaluating -> {
            LoadingScreen()
        }
        is ReviewScreenState.NoTopics -> {
            NoTopicsScreen(onBack = onComplete)
        }
        is ReviewScreenState.Answering -> {
            AnsweringScreen(
                session = currentState.session,
                questionIndex = currentState.questionIndex,
                onSubmit = viewModel::submitAnswer
            )
        }
        is ReviewScreenState.ShowingFeedback -> {
            FeedbackScreen(
                session = currentState.session,
                questionIndex = currentState.questionIndex,
                onContinue = viewModel::continueToNextQuestion
            )
        }
        is ReviewScreenState.Results -> {
            ResultsScreen(
                session = currentState.session,
                hasMoreTopics = currentState.hasMoreTopics,
                onContinue = viewModel::submitAndContinue,
                onFinish = onComplete
            )
        }
        is ReviewScreenState.Complete -> {
            CompleteScreen(onFinish = onComplete)
        }
        is ReviewScreenState.Error -> {
            ErrorScreen(
                message = currentState.message,
                onRetry = { /* Retry logic */ },
                onBack = onComplete
            )
        }
    }
}

@Composable
private fun AnsweringScreen(
    session: ReviewSession,
    questionIndex: Int,
    onSubmit: (String) -> Unit
) {
    var answerText by remember { mutableStateOf("") }
    val question = session.questions[questionIndex]

    Scaffold(
        topBar = {
            TopAppBar(title = { Text("Review") })
        }
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(16.dp)
        ) {
            // Topic header
            Card(
                modifier = Modifier.fillMaxWidth(),
                colors = CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.primaryContainer
                )
            ) {
                Column(
                    modifier = Modifier.padding(16.dp),
                    horizontalAlignment = Alignment.CenterHorizontally
                ) {
                    Text(
                        text = session.keywords.joinToString(", "),
                        style = MaterialTheme.typography.titleMedium,
                        textAlign = TextAlign.Center
                    )
                    Text(
                        text = "Question ${questionIndex + 1} of 3",
                        style = MaterialTheme.typography.bodySmall
                    )
                }
            }

            Spacer(modifier = Modifier.height(16.dp))

            // Question
            Card(modifier = Modifier.fillMaxWidth()) {
                Column(modifier = Modifier.padding(16.dp)) {
                    Text(
                        text = "Question",
                        style = MaterialTheme.typography.labelLarge,
                        color = MaterialTheme.colorScheme.primary
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        text = question.question,
                        style = MaterialTheme.typography.bodyLarge
                    )
                }
            }

            Spacer(modifier = Modifier.height(16.dp))

            // Answer input
            OutlinedTextField(
                value = answerText,
                onValueChange = { answerText = it },
                label = { Text("Your Answer") },
                modifier = Modifier
                    .fillMaxWidth()
                    .weight(1f),
                minLines = 5
            )

            Spacer(modifier = Modifier.height(16.dp))

            // Submit button
            Button(
                onClick = { onSubmit(answerText) },
                modifier = Modifier.fillMaxWidth(),
                enabled = answerText.isNotBlank()
            ) {
                Text("Submit Answer")
            }

            Spacer(modifier = Modifier.height(8.dp))

            // Rating conversion table
            Card(
                modifier = Modifier.fillMaxWidth(),
                colors = CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.surfaceVariant
                )
            ) {
                Text(
                    text = "90%+ → Easy | 70-90% → Good | 60-70% → Hard | <60% → Again",
                    modifier = Modifier.padding(12.dp),
                    style = MaterialTheme.typography.bodySmall,
                    textAlign = TextAlign.Center
                )
            }
        }
    }
}

@Composable
private fun FeedbackScreen(
    session: ReviewSession,
    questionIndex: Int,
    onContinue: () -> Unit
) {
    val question = session.questions[questionIndex]
    val evaluation = question.evaluation!!

    Scaffold(
        topBar = {
            TopAppBar(title = { Text("Feedback") })
        }
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(16.dp)
        ) {
            // Question
            Text(
                text = "Question ${questionIndex + 1} of 3",
                style = MaterialTheme.typography.labelLarge
            )
            Spacer(modifier = Modifier.height(4.dp))
            Text(
                text = question.question,
                style = MaterialTheme.typography.bodyMedium
            )

            Spacer(modifier = Modifier.height(16.dp))

            // Your answer
            Card(modifier = Modifier.fillMaxWidth()) {
                Column(modifier = Modifier.padding(16.dp)) {
                    Text(
                        text = "Your Answer",
                        style = MaterialTheme.typography.labelLarge
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        text = question.answer ?: "",
                        style = MaterialTheme.typography.bodyMedium
                    )
                }
            }

            Spacer(modifier = Modifier.height(16.dp))

            // Evaluation
            val ratingText = when {
                evaluation.score >= 90 -> "Easy"
                evaluation.score >= 70 -> "Good"
                evaluation.score >= 60 -> "Hard"
                else -> "Again"
            }

            val ratingColor = when {
                evaluation.score >= 90 -> Color(0xFF00BCD4) // Cyan
                evaluation.score >= 70 -> Color(0xFF4CAF50) // Green
                evaluation.score >= 60 -> Color(0xFFFF9800) // Orange
                else -> Color(0xFFF44336) // Red
            }

            Card(
                modifier = Modifier.fillMaxWidth(),
                colors = CardDefaults.cardColors(
                    containerColor = ratingColor.copy(alpha = 0.1f)
                )
            ) {
                Column(modifier = Modifier.padding(16.dp)) {
                    Row(
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Icon(
                            imageVector = when {
                                evaluation.score >= 90 -> Icons.Default.SentimentVerySatisfied
                                evaluation.score >= 70 -> Icons.Default.SentimentSatisfied
                                evaluation.score >= 60 -> Icons.Default.SentimentNeutral
                                else -> Icons.Default.SentimentDissatisfied
                            },
                            contentDescription = null,
                            tint = ratingColor,
                            modifier = Modifier.size(32.dp)
                        )
                        Spacer(modifier = Modifier.width(16.dp))
                        Column {
                            Text(
                                text = "Score: ${evaluation.score.toInt()}/100",
                                style = MaterialTheme.typography.titleLarge,
                                color = ratingColor
                            )
                            Text(
                                text = ratingText,
                                style = MaterialTheme.typography.titleMedium,
                                color = ratingColor
                            )
                        }
                    }

                    Spacer(modifier = Modifier.height(12.dp))
                    Divider()
                    Spacer(modifier = Modifier.height(12.dp))

                    Text(
                        text = "Feedback",
                        style = MaterialTheme.typography.labelLarge
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        text = evaluation.feedback,
                        style = MaterialTheme.typography.bodyMedium
                    )
                }
            }

            Spacer(modifier = Modifier.weight(1f))

            Button(
                onClick = onContinue,
                modifier = Modifier.fillMaxWidth()
            ) {
                Text(if (questionIndex < 2) "Next Question" else "See Results")
            }
        }
    }
}

@Composable
private fun ResultsScreen(
    session: ReviewSession,
    hasMoreTopics: Boolean,
    onContinue: () -> Unit,
    onFinish: () -> Unit
) {
    val averageScore = session.averageScore()
    val rating = session.finalRating()
    val ratingText = when (rating) {
        4 -> "Easy"
        3 -> "Good"
        2 -> "Hard"
        else -> "Again"
    }

    Scaffold(
        topBar = {
            TopAppBar(title = { Text("Review Complete") })
        }
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(16.dp),
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            Text(
                text = session.keywords.joinToString(", "),
                style = MaterialTheme.typography.titleLarge,
                textAlign = TextAlign.Center
            )

            Spacer(modifier = Modifier.height(24.dp))

            // Individual scores
            session.questions.forEachIndexed { index, question ->
                question.evaluation?.let { eval ->
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween
                    ) {
                        Text("Question ${index + 1}")
                        Text(
                            text = "${eval.score.toInt()}/100",
                            fontWeight = FontWeight.Bold
                        )
                    }
                    Spacer(modifier = Modifier.height(8.dp))
                }
            }

            Spacer(modifier = Modifier.height(16.dp))
            Divider()
            Spacer(modifier = Modifier.height(16.dp))

            // Average
            Text(
                text = "Average Score",
                style = MaterialTheme.typography.labelLarge
            )
            Text(
                text = "${averageScore.toInt()}%",
                style = MaterialTheme.typography.displayLarge,
                fontWeight = FontWeight.Bold
            )

            Spacer(modifier = Modifier.height(8.dp))

            Text(
                text = "Rating: $ratingText",
                style = MaterialTheme.typography.titleLarge
            )

            Spacer(modifier = Modifier.weight(1f))

            if (hasMoreTopics) {
                Button(
                    onClick = onContinue,
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Text("Next Topic")
                }
            } else {
                Button(
                    onClick = onFinish,
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Text("Finish Session")
                }
            }
        }
    }
}
```

---

## 5. Testing & Verification

### 5.1 Manual Test Cases

**Test Case 1: Add Topic**
```
1. Open app
2. Tap + button
3. Enter: "LSTM, neural networks"
4. Tap "Add Topic"
Expected: Topic appears in list, due in ~24 hours
```

**Test Case 2: Review Flow**
```
1. Wait until topic is due (or modify database)
2. Tap "Start Review"
3. Answer all 3 questions
4. Verify scores are calculated correctly
5. Verify next due date updates
Expected: Average score correct, FSRS calculates proper interval
```

**Test Case 3: Score Conversion**
```
Input scores: 75, 82, 91
Average: 82.7%
Expected rating: Good (3)
Next interval: ~5-10 days
```

### 5.2 Unit Tests

```kotlin
class FSRSSchedulerTest {
    private val scheduler = FSRSScheduler()

    @Test
    fun `new topic rated Good should schedule in ~2-3 days`() {
        val result = scheduler.calculateNextReview(
            rating = 3,
            currentState = null,
            elapsedDays = 0
        )

        assertTrue(result.intervalDays in 2.0..3.0)
    }

    @Test
    fun `rating Again reduces stability`() {
        val state = FSRSScheduler.MemoryState(10.0, 5.0)
        val result = scheduler.calculateNextReview(
            rating = 1,
            currentState = state,
            elapsedDays = 5
        )

        assertTrue(result.newStability < state.stability)
    }
}

class ScoreConversionTest {
    @Test
    fun `score 90 and above converts to Easy`() {
        assertEquals(4, scoreToRating(90.0))
        assertEquals(4, scoreToRating(95.0))
        assertEquals(4, scoreToRating(100.0))
    }

    @Test
    fun `score 70-89 converts to Good`() {
        assertEquals(3, scoreToRating(70.0))
        assertEquals(3, scoreToRating(80.0))
        assertEquals(3, scoreToRating(89.9))
    }
}
```

---

## 6. Implementation Timeline

**Week 1-2: Foundation**
- Setup project, dependencies
- Database entities and DAOs
- Test database operations

**Week 3-4: Network & LLM**
- Retrofit setup
- Groq API integration
- Test question generation and evaluation
- Implement FSRS scheduler

**Week 5-6: Domain Layer**
- Use cases
- Repository implementations
- Unit tests

**Week 7-9: UI - Basic Screens**
- Home screen
- Add topic screen
- Settings screen
- Navigation

**Week 10-12: UI - Review Screen**
- Review flow state machine
- Question/answer UI
- Feedback display
- Results screen

**Week 13-14: Polish & Testing**
- Error handling
- Loading states
- Edge cases
- UI polish
- End-to-end testing

**Total: 14 weeks / ~3.5 months**

---

## 7. Success Criteria

- [ ] Can add topics with keywords
- [ ] Topics appear in home screen
- [ ] Review session works end-to-end
- [ ] LLM generates 3 unique questions
- [ ] LLM evaluates answers with 0-100 scores
- [ ] Score → rating conversion is correct (90%+=Easy, etc.)
- [ ] FSRS schedules next reviews properly
- [ ] Statistics show accurate data
- [ ] App works offline (except LLM calls)
- [ ] No crashes on error cases
- [ ] Clean, intuitive UI

---

## 8. Key Resources

- **Groq API Docs**: https://console.groq.com/docs
- **FSRS Algorithm**: https://github.com/open-spaced-repetition/fsrs4anki/wiki
- **Jetpack Compose**: https://developer.android.com/jetpack/compose
- **Room Database**: https://developer.android.com/training/data-storage/room
- **Material Design 3**: https://m3.material.io/

---

This specification is complete and self-contained. Any developer (or AI agent) can pick it up and build the Android app without needing any additional context about the original CLI app.
