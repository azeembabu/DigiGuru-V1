import * as React from 'react';
import { useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';
import { CheckCircle2, RotateCcw } from 'lucide-react';
import { SectionCard } from './SectionCard';
import { ReadOnlyField } from './ReadOnlyField';
import { Button } from '../ui/Button';
import { Input } from '../ui/Input';
import { profileSchema, type ProfileInput } from '../../lib/validations';
import { getApiErrorMessage } from '../../lib/api';
import type { StudentProfile, ProfileUpdate } from '../../types/profile';

interface Props {
  profile: StudentProfile;
  onSave: (update: ProfileUpdate) => Promise<unknown>;
}

/**
 * Personal information. Full Name and Phone Number are editable; Email and
 * Roll Number are protected and shown as static, read-only text.
 *
 * Which fields are actually editable is decided by the backend's
 * `editableFields` list — the form does not assume a field is editable merely
 * because it is rendered here.
 */
export function PersonalInformationCard({ profile, onSave }: Props) {
  const [serverError, setServerError] = React.useState<string | null>(null);
  const [saved, setSaved] = React.useState(false);

  const canEditName = profile.editableFields.includes('fullName');
  const canEditPhone = profile.editableFields.includes('phoneNumber');

  const {
    register,
    handleSubmit,
    reset,
    formState: { errors, isSubmitting, isDirty },
  } = useForm<ProfileInput>({
    resolver: zodResolver(profileSchema),
    defaultValues: {
      fullName: profile.fullName,
      phoneNumber: profile.phoneNumber,
    },
  });

  // A fresh profile (initial load or after a save) becomes the new baseline so
  // "unsaved changes" reflects reality.
  React.useEffect(() => {
    reset({ fullName: profile.fullName, phoneNumber: profile.phoneNumber });
  }, [profile.fullName, profile.phoneNumber, reset]);

  async function onSubmit(values: ProfileInput) {
    setServerError(null);
    setSaved(false);
    try {
      // Send only the fields this user is allowed to change.
      const update: ProfileUpdate = {};
      if (canEditName) update.fullName = values.fullName;
      if (canEditPhone) update.phoneNumber = values.phoneNumber;

      await onSave(update);
      setSaved(true);
    } catch (err) {
      setServerError(getApiErrorMessage(err, 'Could not save your changes. Please try again.'));
    }
  }

  const noEditableFields = !canEditName && !canEditPhone;

  return (
    <SectionCard
      title="Personal Information"
      description="Your contact details. Roll Number is your permanent identity and cannot be changed."
    >
      <form onSubmit={handleSubmit(onSubmit)} noValidate className="space-y-5">
        {serverError && (
          <div
            className="rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800"
            role="alert"
          >
            {serverError}
          </div>
        )}

        {saved && (
          <div
            className="flex items-center gap-2 rounded-lg border border-green-200 bg-green-50 px-4 py-3 text-sm text-green-800"
            role="status"
          >
            <CheckCircle2 size={16} aria-hidden />
            Your changes have been saved.
          </div>
        )}

        {canEditName ? (
          <Input
            label="Full Name"
            autoComplete="name"
            error={errors.fullName?.message}
            {...register('fullName')}
          />
        ) : (
          <ReadOnlyField label="Full Name" value={profile.fullName} />
        )}

        <ReadOnlyField
          label="Email Address"
          value={profile.email}
          hint="Contact your Learner Support Centre to change your registered email."
        />

        {canEditPhone ? (
          <Input
            label="Phone Number"
            type="tel"
            autoComplete="tel"
            placeholder="+91 98470 00000"
            error={errors.phoneNumber?.message}
            {...register('phoneNumber')}
          />
        ) : (
          <ReadOnlyField label="Phone Number" value={profile.phoneNumber} />
        )}

        <ReadOnlyField
          label="Roll Number"
          value={profile.rollNumber}
          hint="Roll Number is set at admission and identifies your academic record."
        />

        {!noEditableFields && (
          <div className="flex items-center gap-3 border-t border-slate-100 pt-5">
            <Button type="submit" loading={isSubmitting} disabled={!isDirty}>
              Save Changes
            </Button>
            <Button
              type="button"
              variant="ghost"
              onClick={() => {
                reset();
                setServerError(null);
                setSaved(false);
              }}
              disabled={!isDirty || isSubmitting}
            >
              <RotateCcw size={15} aria-hidden />
              Reset
            </Button>
          </div>
        )}
      </form>
    </SectionCard>
  );
}
