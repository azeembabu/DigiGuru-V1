import { CourseBlocks } from "@/components/admin/academic/CourseBlocks";

export default async function CourseBlocksPage({
  params,
}: {
  params: Promise<{ courseId: string }>;
}) {
  const { courseId } = await params;
  return <CourseBlocks courseId={courseId} />;
}
