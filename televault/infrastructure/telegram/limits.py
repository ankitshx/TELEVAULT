from dataclasses import dataclass

FREE_TIER_MAX_BYTES = 2000 * 1024 * 1024  # 2 GB
PREMIUM_TIER_MAX_BYTES = 4000 * 1024 * 1024  # 4 GB


@dataclass(frozen=True)
class AccountLimits:
    is_premium: bool = False

    @property
    def max_upload_bytes(self) -> int:
        return PREMIUM_TIER_MAX_BYTES if self.is_premium else FREE_TIER_MAX_BYTES

    def exceeds_limit(self, file_size: int) -> bool:
        return file_size > self.max_upload_bytes
