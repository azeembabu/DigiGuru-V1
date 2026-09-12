import { BlockDetail } from "@/components/admin/academic/BlockDetail";

export default async function BlockDetailPage({
  params,
}: {
  params: Promise<{ blockId: string }>;
}) {
  const { blockId } = await params;
  return <BlockDetail blockId={blockId} />;
}
