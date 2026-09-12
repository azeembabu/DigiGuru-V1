import { StudentDetail } from "@/components/admin/people/StudentDetail";

// Next 16 hands route params to a page as a promise; the id is the only thing
// that crosses into the URL — never a name or phone number.
export default async function AdminStudentPage({
  params,
}: {
  params: Promise<{ studentId: string }>;
}) {
  const { studentId } = await params;
  return <StudentDetail studentId={studentId} />;
}
