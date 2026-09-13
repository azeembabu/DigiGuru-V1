import { StudentReportDetail } from "@/components/admin/reports/StudentReportDetail";

export default async function StudentReportPage({
  params,
}: {
  params: Promise<{ studentId: string }>;
}) {
  const { studentId } = await params;
  return <StudentReportDetail studentId={studentId} />;
}
