/**
 * L4 Guest Code: Confidential AI Inference
 *
 * This code demonstrates how L4 Frozen Realm enables privacy-preserving AI:
 *
 * Scenario: A medical AI assistant where:
 * - User's medical data (encrypted) transits L1-L3
 * - AI API key (encrypted) transits L1-L3
 * - Both are decrypted ONLY in L4
 * - AI prompt constructed in L4 (never visible to L1-L3)
 * - "API call" simulated (real version would use fetch injected as capability)
 * - Result encrypted before returning
 *
 * Security guarantee: Sensitive medical data NEVER exists in plaintext
 * outside the Frozen Realm, even though it travels through potentially
 * compromised L1-L3 layers.
 *
 * Available capabilities:
 * - log, encryptedPayload, encryptedSecret, enclaveKey
 * - crypto.decrypt, crypto.encrypt, crypto.hash
 * - utils.parseJSON, utils.stringifyJSON
 *
 * @author OutLayer Team + Hermes Enclave
 */

return (async function() {
  log('🧠 L4 Confidential AI Inference Demo Starting...');

  try {
    // ================================================================
    // STEP 1: Decrypt AI API key (OpenAI, Anthropic, etc.)
    // ================================================================

    log('Step 1: Decrypting AI API key...');

    const apiKey = await crypto.decrypt(encryptedSecret, enclaveKey);

    log(`✓ API key decrypted: ${apiKey.slice(0, 8)}${'*'.repeat(24)}`);
    log('  (API key exists ONLY in L4, never in L1-L3!)');

    // ================================================================
    // STEP 2: Decrypt patient medical data
    // ================================================================

    log('Step 2: Decrypting patient medical data...');

    const medicalDataJSON = await crypto.decrypt(encryptedPayload, enclaveKey);
    const medicalData = utils.parseJSON(medicalDataJSON);

    log(`✓ Medical data decrypted:`);
    log(`    Patient: ${medicalData.patientId}`);
    log(`    Symptoms: ${medicalData.symptoms.join(', ')}`);
    log(`    Medical History: ${medicalData.history.length} conditions`);
    log(`    Medications: ${medicalData.medications.join(', ')}`);
    log('  (PHI/PII exists ONLY in L4 scope!)');

    // ================================================================
    // STEP 3: Construct AI prompt (sensitive, never leaves L4)
    // ================================================================

    log('Step 3: Constructing confidential AI prompt...');

    const prompt = `You are a medical AI assistant. Based on the following patient data, provide a preliminary assessment:

Patient ID: ${medicalData.patientId}
Age: ${medicalData.age}
Symptoms: ${medicalData.symptoms.join(', ')}
Medical History: ${medicalData.history.join(', ')}
Current Medications: ${medicalData.medications.join(', ')}

Please provide:
1. Possible differential diagnoses
2. Recommended tests
3. Warning signs to watch for
4. When to seek immediate care

Note: This is not a replacement for professional medical advice.`;

    log(`✓ Prompt constructed (${prompt.length} chars)`);
    log('  (Prompt contains PHI and NEVER leaves L4!)');

    // ================================================================
    // STEP 4: Simulate AI API call
    // ================================================================

    log('Step 4: Simulating AI inference...');

    // In production, this would be:
    // const response = await fetch('https://api.openai.com/v1/chat/completions', {
    //   method: 'POST',
    //   headers: { 'Authorization': `Bearer ${apiKey}` },
    //   body: JSON.stringify({ model: 'gpt-4', messages: [{ role: 'user', content: prompt }] })
    // });

    // For demo, simulate AI response
    const simulatedAIResponse = {
      model: 'gpt-4-simulated',
      choices: [{
        message: {
          role: 'assistant',
          content: `Based on the patient's symptoms (${medicalData.symptoms.join(', ')}) and history of ${medicalData.history[0]}, here's my assessment:

1. **Differential Diagnoses:**
   - Most likely: Condition A related to ${medicalData.symptoms[0]}
   - Consider: Condition B (given history of ${medicalData.history[0]})
   - Rule out: Condition C

2. **Recommended Tests:**
   - Blood work: CBC, metabolic panel
   - Imaging: X-ray or CT scan if symptoms persist
   - Specialist referral: Cardiology consultation

3. **Warning Signs:**
   - Severe chest pain or shortness of breath
   - Sudden weakness or numbness
   - High fever (>102°F)

4. **Seek Immediate Care If:**
   - Symptoms worsen rapidly
   - New severe symptoms develop
   - Existing conditions become unstable

*This assessment was generated using encrypted medical data that never left the secure enclave.*`
        }
      }],
      usage: {
        prompt_tokens: Math.floor(prompt.length / 4),
        completion_tokens: 150,
        total_tokens: Math.floor(prompt.length / 4) + 150,
      },
      apiKeyUsed: `${apiKey.slice(0, 8)}***`,
      securityNote: 'API key and PHI decrypted only in L4 Frozen Realm'
    };

    log(`✓ AI inference complete`);
    log(`    Tokens used: ${simulatedAIResponse.usage.total_tokens}`);

    // ================================================================
    // STEP 5: Create response (strip sensitive data, keep assessment)
    // ================================================================

    log('Step 5: Preparing encrypted response...');

    // Create sanitized response (removes raw PHI, keeps assessment)
    const sanitizedResponse = {
      patientId: medicalData.patientId, // ID is okay to return
      assessment: simulatedAIResponse.choices[0].message.content,
      model: simulatedAIResponse.model,
      tokensUsed: simulatedAIResponse.usage.total_tokens,
      processedAt: 'L4-Frozen-Realm',
      securityGuarantees: {
        apiKeyExposedToL1_L3: false,
        phiExposedToL1_L3: false,
        promptExposedToL1_L3: false,
        encryptionUsed: 'AES-GCM-256',
        plaintextExistedIn: ['L4 only'],
      },
      metadata: {
        symptomsAnalyzed: medicalData.symptoms.length,
        historyConsidered: medicalData.history.length,
        medicationsReviewed: medicalData.medications.length,
        promptLength: prompt.length,
      }
    };

    const responseJSON = utils.stringifyJSON(sanitizedResponse);

    // Encrypt before returning (L4 → L3 → L2 → L1)
    const encryptedResponse = await crypto.encrypt(responseJSON, enclaveKey);

    log('✓ Response encrypted');
    log('🧠 L4 Confidential AI Inference Complete!');
    log('');
    log('Privacy Properties Demonstrated:');
    log('  ✓ API key decrypted ONLY in L4');
    log('  ✓ Patient medical data (PHI) decrypted ONLY in L4');
    log('  ✓ AI prompt constructed ONLY in L4 (never exposed)');
    log('  ✓ L1-L3 saw only encrypted blobs (zero-knowledge ferry)');
    log('  ✓ Result sanitized before encryption');
    log('');
    log('Use Cases Enabled:');
    log('  → Privacy-preserving medical AI');
    log('  → Confidential document analysis');
    log('  → Secure personal assistant');
    log('  → Encrypted knowledge base queries');

    return encryptedResponse;

  } catch (error) {
    log(`✗ ERROR: ${error.message}`);
    throw error;
  }
})();
