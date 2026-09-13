import { SemesterCourses } from "@/components/admin/academic/SemesterCourses";

export default async function SemesterCoursesPage({
  params,
}: {
  params: Promise<{ semesterId: string }>;
}) {
  const { semesterId } = await params;
  return <SemesterCourses semesterId={semesterId} />;
}
