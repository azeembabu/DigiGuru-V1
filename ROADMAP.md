# DIGI GURU - STEP-BY-STEP ROADMAP
### Build it like LEGO - One Block at a Time
> **For +2 Students:** Just follow Step 1, then Step 2, then Step 3. Don't jump.

---

## TOTAL PICTURE - 6 PHASES IN 14 WEEKS

```mermaid
flowchart TD
    A["🟢 START<br/>Your Idea"] --> B["STEP 1<br/>📋 Planning<br/>Week 1"]
    B --> C["STEP 2<br/>🏗️ Foundation<br/>Week 2-3"]
    C --> D["STEP 3<br/>🏫 Classroom MVP<br/>Week 4-7"]
    D --> E["STEP 4<br/>🎙️ Voice & Brain<br/>Week 8-10"]
    E --> F["STEP 5<br/>📚 Study Tools<br/>Week 11-12"]
    F --> G["STEP 6<br/>👑 Admin & Launch<br/>Week 13-14"]
    G --> H["🎉 FINISH<br/>Digi Guru LIVE!"]

    style A fill:#4CAF50,color:#fff,stroke:#2E7D32,stroke-width:3px
    style H fill:#FF9800,color:#fff,stroke:#E65100,stroke-width:3px
    style B fill:#E3F2FD,stroke:#1976D2,stroke-width:2px
    style C fill:#FFF3E0,stroke:#EF6C00,stroke-width:2px
    style D fill:#F3E5F5,stroke:#7B1FA2,stroke-width:2px
    style E fill:#E8F5E9,stroke:#388E3C,stroke-width:2px
    style F fill:#FFFDE7,stroke:#FBC02D,stroke-width:2px
    style G fill:#FCE4EC,stroke:#C2185B,stroke-width:2px
```

---

## DETAILED FLOWCHART - WHAT TO DO INSIDE EACH STEP

```mermaid
flowchart TD
    subgraph PHASE1 ["STEP 1: PLANNING (Week 1) - Like Drawing House Plan"]
        P1A["1.1 Write REQUIREMENT<br/>(This file you are reading) ✅ Done"]
        P1B["1.2 Draw Screens on Paper<br/>Login Page, Classroom Page, Admin Page"]
        P1C["1.3 Design Database Tables<br/>8 Registers we listed"]
        P1A --> P1B --> P1C
    end

    subgraph PHASE2 ["STEP 2: FOUNDATION (Week 2-3) - Like Building Pillars"]
        P2A["2.1 Create Database<br/>PostgreSQL - Make 8 Tables"]
        P2B["2.2 Build STUDENT Signup/Login<br/>Roll No + Program + Sem + LSC + Phone"]
        P2C["2.3 Build Remember Me<br/>Token valid 30 days"]
        P2D["2.4 Build ADMIN Login<br/>Separate Website!"]
        P2A --> P2B --> P2C --> P2D
    end

    subgraph PHASE3 ["STEP 3: CLASSROOM MVP (Week 4-7) - The Main Hall"]
        P3A["3.1 Build Whiteboard<br/>Use tldraw - Don't code from Zero"]
        P3B["3.2 Connect Gemini Text Chat<br/>Student types doubt -> Gemini replies"]
        P3C["3.3 Add Textbook RAG<br/>Upload PDF -> AI answers ONLY from it"]
        P3D["3.4 Add Memory System<br/>Knows Name + Yesterday Revision"]
        P3E["3.5 Add Course Switch Button<br/>On Whiteboard"]
        P3A --> P3B --> P3C --> P3D --> P3E
    end

    subgraph PHASE4 ["STEP 4: VOICE & BRAIN (Week 8-10) - Give Teacher a Voice"]
        P4A["4.1 Add Mic Button<br/>Student can SPEAK"]
        P4B["4.2 Add Gemini Voice Output<br/>Teacher SPEAKS back"]
        P4C["4.3 Build Voice Cache<br/>Save common Q voices to reuse"]
        P4D["4.4 Build Adaptive Logic<br/>Beginner -> Slow/Fast based on YOU"]
        P4E["4.5 Add 20-Min Timer + Goodbye<br/>Auto-end class"]
        P4A --> P4B --> P4C --> P4D --> P4E
    end

    subgraph PHASE5 ["STEP 5: STUDY TOOLS (Week 11-12) - Notes & Exams"]
        P5A["5.1 Auto Notes Generator"]
        P5B["5.2 Flashcards (5 Q/A)"]
        P5C["5.3 Mind Map (Diagram)"]
        P5D["5.4 Visual Notes PDF (Download)"]
        P5E["5.5 Unit Test + Oral Exam<br/>Marks + Mistake Highlighting"]
        P5A --> P5B --> P5C --> P5D --> P5E
    end

    subgraph PHASE6 ["STEP 6: ADMIN & LAUNCH (Week 13-14) - Principal Office + Opening"]
        P6A["6.1 Admin Dashboard<br/>See all Students & Progress"]
        P6B["6.2 Textbook Upload Tool<br/>PDF -> Paragraph/Page No"]
        P6C["6.3 Test with 10 Students<br/>Find bugs"]
        P6D["6.4 Deploy 2 Websites<br/>student.digiguru.com<br/>admin.digiguru.com"]
        P6A --> P6B --> P6C --> P6D
    end

    PHASE1 --> PHASE2 --> PHASE3 --> PHASE4 --> PHASE5 --> PHASE6
```

---

## SIMPLE TABLE ROADMAP - FOR YOUR NOTEBOOK

Copy this into your notebook. Tick each box when done.

### 🟦 STEP 1: PLANNING - WEEK 1 (Day 1 to Day 7)
| No | Task | Simple Meaning | Done? |
|----|------|----------------|-------|
| 1.1 | Finalize REQUIREMENT.md | This document is your bible | ✅ |
| 1.2 | Draw 5 Screens on Paper | Login, Signup, Classroom, Whiteboard, Admin Dashboard | ☐ |
| 1.3 | List All Database Tables | Write 8 table names on paper with columns | ☐ |
| 1.4 | Choose Technology | Next.js + Python FastAPI + PostgreSQL + Gemini | ☐ |
| 1.5 | Create GitHub Repo | Make 2 folders: `student-app` and `admin-app` | ☐ |

> **End of Week 1 Check:** Can you show your 5 paper sketches to a friend and they understand?

### 🟧 STEP 2: FOUNDATION - WEEK 2 & 3 (Day 8 to Day 21)
| No | Task | How to Do (Like Recipe) | Done? |
|----|------|-------------------------|-------|
| 2.1 | Setup Database | Install PostgreSQL. Create 8 empty tables. | ☐ |
| 2.2 | Student Signup Page | Form with 5 fields (Roll No, Program, Sem, LSC, Phone) + `Save to DB` | ☐ |
| 2.3 | Student Login Page | Roll No + Password check from DB | ☐ |
| 2.4 | Remember Me Logic | If ticked, save Token for 30 days using Cookie | ☐ |
| 2.5 | Admin Website (Separate!) | New project, new design, Login with Email/Password. NO link from Student app. | ☐ |
| 2.6 | Test | Create 3 fake students and try login/logout | ☐ |

> **Analogy:** Like building the foundation and walls of a house first. No painting yet.

### 🟪 STEP 3: CLASSROOM MVP - WEEK 4 to 7 (Day 22 to Day 49)
| No | Task | How to Do | Done? |
|----|------|-----------|-------|
| 3.1 | Whiteboard | Install `tldraw` package. Show empty whiteboard on screen. | ☐ |
| 3.2 | Text Chat with Gemini | Take student doubt (text) -> Send to Gemini API -> Show reply | ☐ |
| 3.3 | Textbook RAG (MOST IMPORTANT) | Upload 1 PDF (e.g., BA Malayalam Book) -> Split by Page No/Para No -> Store in Vector DB -> When doubt comes, search PDF and give only that text to Gemini | ☐ |
| 3.4 | Student Memory | On Login, fetch last class data from DB -> Send to Gemini as "This is Anu, he studied Chapter 2 yesterday" -> Gemini greets with name | ☐ |
| 3.5 | Revision Logic | Write `if` condition: `if last_class == yesterday => give 1-min revision` else if `same day same chapter => 20-sec revision` | ☐ |
| 3.6 | Course Switch | Add Dropdown Button on Whiteboard + Listen voice command "Change course to..." | ☐ |

> **Test:** Login as Anu (BA Malayalam, Sem 3). Ask "What is Chapter 2?". AI should reply ONLY from PDF + call you Anu.

### 🟩 STEP 4: VOICE & BRAIN - WEEK 8 to 10 (Day 50 to Day 70)
| No | Task | How to Do | Done? |
|----|------|-----------|-------|
| 4.1 | Mic Button | Add 🎤 button. Use `MediaRecorder` to record voice -> Send to Google Speech-to-Text | ☐ |
| 4.2 | Show "Listening..." | When student speaks, show wave animation (like WhatsApp recording) | ☐ |
| 4.3 | AI Voice Reply | Gemini generates text -> Send to Google Text-to-Speech -> Play audio on speaker | ☐ |
| 4.4 | Voice Cache | Create `voice_cache` table. Logic: `if question already exists in cache? Play saved file : Generate new and save` | ☐ |
| 4.5 | Adaptive Learning | Start everyone at Level 1. After each Q, if student correct -> Level++. If wrong -> stay Level 1. Change language difficulty based on level. | ☐ |
| 4.6 | 20-Min Timer | Show countdown timer top-right. At 18:00 show warning. At 20:00 auto-save and show "Goodbye" | ☐ |
| 4.7 | Smart Doubt Repeat | If student says "repeat please" -> Gemini repeats same explanation patiently | ☐ |

### 🟨 STEP 5: STUDY TOOLS - WEEK 11 & 12 (Day 71 to Day 84)
| No | Task | How to Do | Done? |
|----|------|-----------|-------|
| 5.1 | Notes Generator | After class, send transcript to Gemini: "Make summary notes" -> Save and show | ☐ |
| 5.2 | Flashcards | Prompt: "Make 5 Q/A flashcards from today's class" -> Show as flip cards | ☐ |
| 5.3 | Mind Map | Get JSON from Gemini -> Draw using `React Flow` (dots and lines diagram) | ☐ |
| 5.4 | PDF Export | Take Whiteboard Screenshot + Notes -> Use `jsPDF` library -> Generate `Anu_Chapter3_Notes.pdf` -> Download button | ☐ |
| 5.5 | Unit Test | After Chapter finish -> Generate 10 MCQs from textbook -> Student answers -> Show Score | ☐ |
| 5.6 | Oral Test (IMPORTANT) | Teacher asks Q by voice -> Student answers by voice -> Wait for full silence (1.5 sec) -> Analyze full answer -> Give Marks + Correct Answer + Mistakes in RED + Tip | ☐ |
| 5.7 | Chart/Graph Rule | When topic needs graph: Tool `draw_graph()` -> Draw on whiteboard FIRST -> THEN Gemini starts explaining | ☐ |

### 🟥 STEP 6: ADMIN & LAUNCH - WEEK 13 & 14 (Day 85 to Day 98)
| No | Task | How to Do | Done? |
|----|------|-----------|-------|
| 6.1 | Admin - View Students | Table: Roll No, Name, Program, Sem, LSC, Progress %, Last Login | ☐ |
| 6.2 | Admin - Upload Textbook | Button: Upload PDF -> Backend splits into Paragraph/Page No -> Store in Vector DB | ☐ |
| 6.3 | Admin - Analytics | Graph: How many students studied today, voice cache saved how much money | ☐ |
| 6.4 | Testing Phase | Give to 10 real students, note all bugs, fix them | ☐ |
| 6.5 | Deploy Student App | Host on `student.digiguru.com` (Vercel / Hostinger) | ☐ |
| 6.6 | Deploy Admin App | Host on `admin.digiguru.com` (DIFFERENT domain) | ☐ |
| 6.7 | Final Security Check | Make sure student can NEVER guess or access admin URL by typing /admin | ☐ |

---

## STUDENT JOURNEY FLOWCHART - HOW A STUDENT USES THE APP DAILY

```mermaid
flowchart TD
    S["Student Opens App"] --> L{"Already Logged In?<br/>(Remember Me)"}
    L -- Yes --> W["Welcome, Anu!<br/>Ready for BA Malayalam?"]
    L -- No --> LG["Login Page<br/>Roll No + Password"]
    LG --> W

    W --> C{"What Student Wants?"}
    C -- New Chapter --> NC["Gemini: Welcome to Chapter 3!<br/>Starts from Beginner + Light Joke"]
    C -- Continue Same Chapter Same Day --> SD["Gemini: 20-sec Quick Note<br/>of Morning Class"]
    C -- Came Next Day --> ND["Gemini: 1-Min Revision<br/>of Yesterday + Start New Class"]
    C -- Has Doubt --> D["Student Speaks/Types Doubt<br/>-> Search Textbook -> AI Answers<br/>ONLY from Book"]

    NC --> WB["Whiteboard<br/>Teacher Draws First<br/>Then Explains"]
    SD --> WB
    ND --> WB
    D --> WB

    WB --> Q{"Teacher Asks Check-Q<br/>Every 5 Mins"}
    Q -- Correct --> UP["Level Up!<br/>Teach Faster/Deeper"]
    Q -- Wrong --> STAY["Stay Same Level<br/>Explain Again Patiently"]

    UP --> TMR{"20 Mins Over?"}
    STAY --> TMR

    TMR -- No --> WB
    TMR -- Yes --> ENDC["Teacher: Goodbye, Anu!<br/>Refer Page 42 when free"]
    ENDC --> TOOLS["Auto Generate:<br/>Notes | Flashcards<br/>Mind Map | PDF"]
    TOOLS --> TEST{"Want Test?"}
    TEST -- Oral Test --> ORAL["You Speak Answer<br/>AI Listens -> Full Analysis<br/>Marks + Mistakes in RED + Tip"]
    TEST -- Unit Test --> MCQ["10 MCQs from Textbook"]
    TEST -- No --> BYE["Logout / Close App"]
    ORAL --> BYE
    MCQ --> BYE
```

---

## DECISION FLOWCHART - WHEN TO DO WHAT?

```mermaid
flowchart TD
    Q1{"Student Asks Question"}
    Q1 --> S1{"Is it from Textbook?"}
    S1 -- YES --> A1["Answer from Textbook<br/>with Page No & Para No"]
    S1 -- NO --> A2["Polite Reply:<br/>Outside Textbook Scope"]

    Q2{"Student Says<br/>'Repeat Please'"}
    Q2 --> A3["Repeat Same Explanation<br/>Patiently Like Real Teacher"]

    Q3{"Student Needs<br/>Graph/Chart/Equation?"}
    Q3 --> STEP1["STEP 1: Draw on Whiteboard"]
    STEP1 --> STEP2["STEP 2: Then Start Explaining"]

    Q4{"Question Asked<br/>Before?"}
    Q4 -- YES, Common Q --> PLAY["Play Saved Voice<br/>from Cache (Fast & Cheap)"]
    Q4 -- NO, New Q --> GEN["Generate New Voice<br/>+ Save to Cache for Next Time"]

    Q5{"Class Time?"}
    Q5 -- "< 18 mins" --> CONT["Continue Teaching"]
    Q5 -- "18 mins" --> WARN["Warn: 2 Mins Left"]
    Q5 -- "20 mins" --> STOP["Stop Class<br/>Say Goodbye"]
```

---

## FILE & FOLDER STRUCTURE - WHERE TO PUT CODE?

```
Digi-Guru/
│
├── student-app/                  <-- WEBSITE 1: For Students Only
│   ├── pages/
│   │   ├── signup.js             (Roll No, Program, Sem, LSC, Phone)
│   │   ├── login.js              (Roll No + Password + Remember Me)
│   │   └── classroom.js          (Whiteboard + AI Chat + Mic)
│   └── components/
│       ├── Whiteboard.jsx        (tldraw)
│       └── VoiceRecorder.jsx     (Mic button)
│
├── admin-app/                    <-- WEBSITE 2: For Admin Only (SEPARATE!)
│   ├── pages/
│   │   ├── login.js              (Email + Password)
│   │   └── dashboard.js          (View Students, Upload PDF)
│   └── components/
│       └── TextbookUploader.jsx
│
├── backend-api/                  <-- THE WAITER (Connects Everything)
│   ├── auth/                     (Login/Signup logic)
│   ├── gemini/                   (AI Teacher logic)
│   ├── rag/                      (Textbook Search logic)
│   └── voice-cache/              (Saved Voices)
│
└── database/                     <-- 8 Registers
    ├── students table
    ├── textbooks (Vector DB)
    └── ... (other 6 tables)
```

---

## WEEKLY CHECKLIST - PRINT THIS AND STICK ON WALL

```
WEEK 1  [ ] Planning & Paper Sketches
WEEK 2  [ ] Database Created
WEEK 3  [ ] Student + Admin Login Working
WEEK 4  [ ] Whiteboard Visible
WEEK 5  [ ] Gemini Chat Working (Text Only)
WEEK 6  [ ] Textbook PDF Upload + RAG Working
WEEK 7  [ ] Memory + Revision Logic Working
WEEK 8  [ ] Mic + Voice Working
WEEK 9  [ ] Voice Cache + Adaptive Level
WEEK 10 [ ] 20-Min Timer + Goodbye
WEEK 11 [ ] Notes, Flashcards, Mind Map, PDF
WEEK 12 [ ] Unit Test + Oral Test
WEEK 13 [ ] Admin Dashboard Full
WEEK 14 [ ] Testing + Deploy LIVE!
```

---

## GOLDEN RULES - NEVER FORGET (Like Traffic Rules)

1.  **Admin is INVISIBLE to Student** - Two websites, never one.
2.  **Textbook is GOD** - AI never answers outside it.
3.  **Graph FIRST, Explain LATER** - Always draw before talking.
4.  **Listen FULLY before Marking** - Wait for student to fully finish speaking.
5.  **Always say Goodbye** - Every class ends with Goodbye.
6.  **Roll Number = Identity** - Everything is linked to Roll No, not Name.
7.  **One Student = One World** - Data never mixes.

---

**Made for:** Digi Guru Project
**Date:** 09 Sep 2026
**Folder:** D:\Digi Guru-Sep-2026
**Next Step:** Show this ROADMAP.md to your developer. They will understand exactly what to build day-by-day.
