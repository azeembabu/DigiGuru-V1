import { ProgramDetail } from "@/components/admin/academic/ProgramDetail";

export default async function ProgramDetailPage({
  params,
}: {
  params: Promise<{ programId: string }>;
}) {
  const { programId } = await params;
  return <ProgramDetail programId={programId} />;
}
