from fastapi.testclient import TestClient
import pytest

from televault.presentation.web.app import app


@pytest.fixture
def client():
    return TestClient(app)


def test_web_manifest_endpoints(client: TestClient):
    # Verify manifest
    res = client.get("/api/manifest")
    assert res.status_code == 200
    data = res.json()
    assert "is_valid" in data

    # Generate new manifest
    res_gen = client.post("/api/manifest/generate")
    assert res_gen.status_code == 200
    gen_data = res_gen.json()
    assert gen_data["status"] == "complete"
    assert "generation" in gen_data


def test_web_audit_endpoint(client: TestClient):
    res = client.get("/api/audit")
    assert res.status_code == 200
    data = res.json()
    assert "is_valid" in data
    assert "events" in data


def test_web_doctor_endpoint(client: TestClient):
    res = client.get("/api/doctor?force_fake=true")
    assert res.status_code == 200
    data = res.json()
    assert data["status"] == "complete"
    assert "findings" in data
    assert len(data["findings"]) > 0


def test_web_ai_query_endpoint(client: TestClient):
    payload = {"role": "doctor", "prompt": "Check if any files are degraded."}
    res = client.post("/api/ai/query", json=payload)
    assert res.status_code == 200
    data = res.json()
    assert data["agent_id"] == "vault_doctor"
    assert "Vault Doctor Diagnosis" in data["message"]
