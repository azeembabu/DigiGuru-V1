-- CreateEnum
CREATE TYPE "EnrollmentStatus" AS ENUM ('ACTIVE', 'COMPLETED', 'DROPPED');

-- CreateEnum
CREATE TYPE "SessionStatus" AS ENUM ('IN_PROGRESS', 'COMPLETED', 'ABANDONED');

-- CreateTable
CREATE TABLE "student_courses" (
    "id" TEXT NOT NULL,
    "student_id" TEXT NOT NULL,
    "course_id" TEXT NOT NULL,
    "status" "EnrollmentStatus" NOT NULL DEFAULT 'ACTIVE',
    "assigned_at" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "created_at" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" TIMESTAMP(3) NOT NULL,

    CONSTRAINT "student_courses_pkey" PRIMARY KEY ("id")
);

-- CreateTable
CREATE TABLE "learning_sessions" (
    "id" TEXT NOT NULL,
    "student_id" TEXT NOT NULL,
    "course_id" TEXT NOT NULL,
    "unit_id" TEXT,
    "chapter_id" TEXT,
    "started_at" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "ended_at" TIMESTAMP(3),
    "duration_seconds" INTEGER NOT NULL DEFAULT 0,
    "completion_status" "SessionStatus" NOT NULL DEFAULT 'IN_PROGRESS',
    "created_at" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" TIMESTAMP(3) NOT NULL,
    "student_course_id" TEXT,

    CONSTRAINT "learning_sessions_pkey" PRIMARY KEY ("id")
);

-- CreateIndex
CREATE INDEX "student_courses_student_id_idx" ON "student_courses"("student_id");

-- CreateIndex
CREATE INDEX "student_courses_course_id_idx" ON "student_courses"("course_id");

-- CreateIndex
CREATE INDEX "student_courses_status_idx" ON "student_courses"("status");

-- CreateIndex
CREATE UNIQUE INDEX "student_courses_student_id_course_id_key" ON "student_courses"("student_id", "course_id");

-- CreateIndex
CREATE INDEX "learning_sessions_student_id_started_at_idx" ON "learning_sessions"("student_id", "started_at");

-- CreateIndex
CREATE INDEX "learning_sessions_student_id_course_id_idx" ON "learning_sessions"("student_id", "course_id");

-- CreateIndex
CREATE INDEX "learning_sessions_completion_status_idx" ON "learning_sessions"("completion_status");

-- AddForeignKey
ALTER TABLE "student_courses" ADD CONSTRAINT "student_courses_student_id_fkey" FOREIGN KEY ("student_id") REFERENCES "students"("id") ON DELETE CASCADE ON UPDATE CASCADE;

-- AddForeignKey
ALTER TABLE "student_courses" ADD CONSTRAINT "student_courses_course_id_fkey" FOREIGN KEY ("course_id") REFERENCES "courses"("id") ON DELETE CASCADE ON UPDATE CASCADE;

-- AddForeignKey
ALTER TABLE "learning_sessions" ADD CONSTRAINT "learning_sessions_student_id_fkey" FOREIGN KEY ("student_id") REFERENCES "students"("id") ON DELETE CASCADE ON UPDATE CASCADE;

-- AddForeignKey
ALTER TABLE "learning_sessions" ADD CONSTRAINT "learning_sessions_course_id_fkey" FOREIGN KEY ("course_id") REFERENCES "courses"("id") ON DELETE CASCADE ON UPDATE CASCADE;

-- AddForeignKey
ALTER TABLE "learning_sessions" ADD CONSTRAINT "learning_sessions_student_course_id_fkey" FOREIGN KEY ("student_course_id") REFERENCES "student_courses"("id") ON DELETE SET NULL ON UPDATE CASCADE;
