# DIGI GURU - REQUIREMENT DOCUMENT
### The Complete Guide - Written for a +2 Student Level

> **One Line Idea:** Digi Guru is an Online Tuition Centre where a Real-like AI Teacher teaches YOU personally on a whiteboard, using ONLY your textbook.

---

## 1) WHO WILL USE THIS SYSTEM?

Think of it like a School with 2 Types of People:

| User | Who are they? | What do they do? | Example |
|------|---------------|------------------|---------|
| **1. STUDENT** | You (the learner) | Join class, talk to AI teacher, give exams | Like a student in a tuition class |
| **2. ADMIN** | The Owner / Office Staff | Manages all students, uploads textbooks, sees progress | Like the Principal in office |

> **MOST IMPORTANT RULE:** Student and Admin have **TWO DIFFERENT WEBSITES.**
> Student Website = `student.digiguru.com`
> Admin Website = `admin.digiguru.com`
> A student will **NEVER** see the Admin login button. Not even in small letters. Fully separate.

---

## 2) STUDENT - SIGNUP (First Time Joining)

Imagine you are taking admission in a tuition centre. What details will they ask?

When a new student clicks **"Sign Up"**, the form will ask for 5 THINGS ONLY:

1.  **Roll Number** - Your Unique ID (Example: `BA-ML-2024-0123`). This is your main identity. No two students can have same Roll Number.
2.  **Program** - What are you studying? (Example: BA Malayalam, BCom, BA English)
3.  **Semester** - Which Semester are you in now? (Example: Sem 1, Sem 2, Sem 3)
4.  **LSC** - Learner Support Centre / Your Study Centre Code (Example: LSC- Kochi-01)
5.  **Phone Number** - Your mobile number for contact.

Plus you will create a **Password**.

**What happens backend?** All these 5 details are saved in a Database (like a big Excel sheet stored safely). Roll Number is the Primary Key - means we will find you only with Roll Number.

---

## 3) STUDENT - LOGIN (Daily Entry)

Very simple:

*   Student enters **Roll Number + Password** and clicks Login.
*   There is a small tick box called **[ ] Remember Me**
    *   If you tick it -> Next time you open the website, you are **automatically logged in** for 30 days. No need to type password again (like Instagram).
    *   If you DON'T tick it -> You will be logged out after 8 hours for safety.

**Behind the Scene:** When you login, the server gives you a secret `Token` (like a Hall Ticket). Your phone saves it. Every time you open the app, it shows the Hall Ticket to prove "It's me".

---

## 4) THE AI TEACHER - GEMINI (The Heart of Digi Guru)

This is the most important part. We will use **Google Gemini** (Version 3.1 when available, right now 2.5 Flash Live supports this).

Think of Gemini as a Real Teacher who lives inside the system.

### How will the AI Teacher talk?

1.  **It knows YOU:** When you login, it already knows your Name, Program, Semester, Course. It will say: *"Welcome back, Anu! Ready for today's BA Malayalam class?"* - It will call you by your NAME. No boring "Hello User".
2.  **Polite & Gentle:** It will always talk very politely and sweetly. Not rude.
3.  **No Unnecessary Greetings:** It will not waste time with long hellos. Direct to the point.
4.  **Memory Like a Real Teacher:** If you studied Chapter 2 yesterday, and today you ask "What did we study yesterday?" - it will FIRST give you a **30-second to 1-minute revision** of yesterday, and THEN start today's class.

### Two Types of Revision (Very Important Rule):

*   **Case A - You come NEXT DAY and ask about yesterday:** -> Give 1-minute short revision of yesterday, then start new class.
*   **Case B - You come SAME DAY (Morning studied at 10 AM, again at 6 PM, same chapter):** -> Give only a **20-second** small note of what you studied in the morning.
*   **Case C - You come same day but DIFFERENT CHAPTER:** -> No revision needed. Directly start new chapter.

### How will it Teach?

5.  **Starts from ZERO (Beginner Level):** Always starts as if you know nothing. Makes it easy.
6.  **Uses Light Humor:** At the beginning, it can crack a small, simple joke to make you comfortable.
7.  **Becomes Serious if YOU become Serious:** If you start answering seriously and want serious study, the AI will understand and change to a serious teacher style automatically.
8.  **Adapts to Your Intelligence:** If you are fast, it teaches fast. If you are slow, it teaches slow. It teaches *YOU* according to *YOUR* level. This is called **Adaptive Learning**.
9.  **Asks Questions in Between:** Every 4-5 minutes, it will ask YOU a question - "Did you understand this Anu?" to check if you are listening. It will not go forward until you understand that paragraph.
10. **Teaches Paragraph by Paragraph:** For every paragraph, it will tell you: `Chapter 3 | Paragraph 4 | Page No 42`. So you can also open your real textbook and see.

### Whiteboard + Voice

11. **Whiteboard:** On your screen there is a Whiteboard (like a blackboard in class). Whatever the teacher teaches, he writes/draws there.
12. **Audio Input (You can SPEAK):** You don't need to only type. You can press Mic button and ASK your doubt by speaking. The AI will listen.
13. **Changing Course:** On the whiteboard, there is a button to change course. You can also just SAY - "Sir, change to BA History". Both options (Button + Voice) should work.

---

## 5) TEXTBOOK RULE (Very Strict)

> The AI Teacher will answer **ONLY** from the Textbook provided by Admin.

*   If you ask "What is Photosynthesis from Chapter 2?", it will search the textbook and answer ONLY that.
*   If you ask "Who won the IPL 2026?" (which is outside textbook), it will politely say: *"This question is outside our textbook. Please refer to your textbook for this course."*
*   It will NEVER give answers outside the textbook guidelines.

**How is this done?** All Textbooks are converted to `Vectors` and stored in a `Vector Database`. When you ask a question, the system first searches the textbook, finds the correct Page/Paragraph, and gives ONLY that to Gemini as "Context". Gemini cannot answer beyond that.

---

## 6) THE RECORDED VOICE SYSTEM (To Save Money & Time)

Imagine 1000 students ask the same question: "What is Noun?"

If we call Gemini Voice 1000 times, it costs a lot of money.

**Solution:**
*   First time question "What is Noun?" is asked -> Gemini creates voice -> We SAVE that voice file in Storage (like saving a song).
*   Next 999 times anyone asks the same question -> We directly play the SAVED voice. No need to call Gemini again.

This is called **Voice Caching**. For common questions, we reuse the recorded voice.

---

## 7) CLASS RULES

1.  **20-Minute Class Limit:** One class is only for 20 Minutes. At 18 minutes, teacher says "We will close in 2 minutes". At 20 minutes, class auto-ends. This keeps you focused.
2.  **Proper Closing:** Every single class MUST end with the teacher saying **"Goodbye"** as the last word. Nice manners.
3.  **After Class - Refer to Material:** After saying activity completed, teacher will say "Please also refer to Page No X of your textbook when you have time."

---

## 8) AFTER-CLASS TOOLS (What you get after studying)

After every class, the system will automatically create 4 things for you:

| Tool | What it is? | Simple Meaning |
|------|-------------|----------------|
| **1. Notes** | Summary of today's class | Like your handwritten notes |
| **2. Flashcards** | Small Q & A Cards | For quick revision before exam |
| **3. Mind Map** | Diagram showing connections | Like a family tree of the chapter |
| **4. Visual Notes PDF** | Whiteboard drawings + Notes as PDF | You can Download and Print it |

You can request Notes anytime: "Sir, give me notes for Chapter 2".

---

## 9) TESTS & EXAMS

1.  **Unit Tests:** After finishing one full Unit/Chapter, there is a small test.
2.  **Main Examination Preparation:** The system has full exam pattern questions to practice for your final university exam.
3.  **Oral Answer System (Speaking Test):**
    *   Teacher asks a question verbally.
    *   YOU answer by speaking into the Mic.
    *   System shows **"Listening..."** with a wave animation so you know it is listening.
    *   It waits until you fully finish speaking (waits for 1.5 seconds of silence).
    *   Then it analyzes your FULL answer.
    *   It gives you: **Marks (e.g., 6/10) + Correct Answer + Where YOU Made Mistake (highlighted in red) + Tip to avoid mistake next time.**
    *   It does NOT give marks without analyzing fully.

4.  **Repeating Doubts:** If you say "Sir, please repeat" or "I didn't understand", the teacher will patiently repeat the same explanation like a real teacher, not in a robotic way.

---

## 10) CHARTS, GRAPHS & EQUATIONS (Drawing Rule)

This is a special rule for Science/Maths subjects.

> **Step 1:** First CREATE and DRAW the Chart/Graph/Equation on the whiteboard.
> **Step 2:** ONLY AFTER drawing is complete, START explaining it.

Example: If topic is "Bar Graph of Rainfall", first the Bar Graph appears on whiteboard, then the teacher says "Now see, this bar shows..."

The Text explanation is generated AFTER the visual is ready, not before.

---

## 11) ONE STUDENT = ONE SEPARATE WORLD

If 1000 students use Digi Guru at the same time:
*   Anu's progress, memory, notes, voice are ONLY for Anu.
*   Rahul's progress, memory, notes, voice are ONLY for Rahul.
*   They never mix. Everything is separated by **Roll Number**.

Admin can see all, but students cannot see each other.

---

## 12) ADMIN PORTAL (Separate Website) - What Admin Can Do

Login with Email + Password (Not Roll Number).

1.  **View All Students:** List of all students with Roll No, Program, Sem, LSC.
2.  **Upload Textbooks:** Admin uploads PDF of textbook -> System automatically cuts it into Paragraphs, Page Numbers, and saves it for teaching. This is the ONLY source for AI.
3.  **Manage Courses:** Add new Program, Add new Semester, Add new Course.
4.  **See Progress:** How much each student studied, how many tests passed, time spent.
5.  **See Voice Cache Savings:** How much money saved by reusing voices.
6.  **Manage Exams:** Create Main Exam papers and Unit Tests.
7.  **Never Visible to Student:** No student menu will have "Admin" word.

---

## 13) DATABASE - WHAT WE STORE (Like School Registers)

We need 8 Registers (Tables) in our Database:

1.  `students` - Your admission form (Roll No, Name, Program, Sem, LSC, Phone)
2.  `sessions` - Your Hall Ticket / Login token (For Remember Me)
3.  `textbooks` - The Library (All textbook paragraphs with Page No)
4.  `learning_progress` - Your Progress Card (Which chapter completed, what is your level)
5.  `class_history` - Your Daily Attendance Register (What you studied today, what time, transcript)
6.  `assessments` - Your Exam Marks Register (Marks, mistakes, correct answer)
7.  `voice_cache` - Your Song Collection (Saved common question voices)
8.  `admins` - Staff Register (Separate from students)

---

## 14) NON-FUNCTIONAL REQUIREMENTS (Quality Rules)

*   **Secure:** Passwords are hashed (not saved directly), Login tokens are safe.
*   **Fast:** Voice cache makes answers fast (no waiting).
*   **Private:** One student's data never shows to another student.
*   **Scalable:** Even if 10,000 students login, system should not hang.
*   **Available 24/7:** Student can study at 10 AM or 2 AM anytime.

---

## 15) TECHNOLOGY WE WILL USE (Tools)

You don't need to buy anything. These are software tools developers will use:

*   **Frontend (What YOU see):** Next.js + React (for both Student and Admin websites, but as 2 separate projects)
*   **Backend (The Brain behind):** Python FastAPI or Node.js (connects Frontend to Database and Gemini)
*   **Database (Registers):** PostgreSQL (for student data) + pgvector (for textbook search) + Redis (for Remember Me and Voice Cache fast access)
*   **AI:** Google Gemini 2.5 Flash (Live API for Voice) - Will upgrade to 3.1 when released
*   **Whiteboard:** tldraw or Fabric.js (ready-made drawing tools, we don't build from zero)
*   **Voice to Text:** Google Speech-to-Text / Gemini Live Audio Input
*   **File Storage:** AWS S3 or Google Cloud Storage (to save PDFs and voice files)
*   **Hosting:** Student App and Admin App on TWO DIFFERENT DOMAINS

---

## 16) GLOSSARY (Difficult Words Made Simple)

| Word | Simple Meaning |
|------|----------------|
| **LSC** | Learner Support Centre - Your tuition centre code |
| **RAG** | Search textbook first, then answer. Like open book exam for AI |
| **Vector DB** | A library where textbooks are stored as numbers so AI can search fast |
| **JWT Token** | Your Hall Ticket for login |
| **API** | The waiter who takes your request from Website to Database |
| **Adaptive Learning** | Teaching fast or slow depending on YOUR speed |
| **Voice Cache** | Saved voice songs to reuse and save money |

---

## 17) SUCCESS CRITERIA (How we know it works?)

*   Student can signup with Roll Number and login with Remember Me.
*   AI calls student by name and remembers yesterday's class.
*   AI answers ONLY from textbook.
*   Whiteboard draws graph BEFORE explanation.
*   Oral answer gets marks + mistake highlighting.
*   Class ends exactly at 20 mins with "Goodbye".
*   Admin website is 100% separate and not visible to students.
*   Each student's data is fully separate.

---

**Document Created For:** Digi Guru Project - D:\Digi Guru-Sep-2026
**Date:** 09 Sep 2026
**Version:** 1.0 - For +2 Student Understanding
