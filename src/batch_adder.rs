use ark_bls12_381::G1Affine;
use ark_ec::{AffineCurve};
use ark_ff::Field;
use ark_std::{One, Zero};
// Batch Addition for Multi-Scalar Multiplication
//problem: Given two sets of EC points {Pi},{Qi} ,calculates {Ri = Pi + Qi}
//Native Approach:
//  k = { Q.y-P.y/Q.x-P.x  if P != Q or -Q
//        3*(P.x)^2+a/2P.y if p = Q }
// R.x = k^2-P.x -Q.x
// R.y = k(P.x-R.x)-P.y

//参考https://hackmd.io/1mpavmFmQNWrahBi8mHBjQ
#[derive(Debug)]
pub struct BatchAdder {
    inverse_state: <G1Affine as AffineCurve>::BaseField,
    inverses: Vec<<G1Affine as AffineCurve>::BaseField>,
}

impl BatchAdder {
    pub fn new(max_batch_cnt: usize) -> Self {
        BatchAdder {
            inverse_state: <G1Affine as AffineCurve>::BaseField::one(),
            inverses: vec![<G1Affine as AffineCurve>::BaseField::one(); max_batch_cnt],
        }
    }

    /// Batch add vector dest and src, the results will be stored in dest, i.e. dest[i] = dest[i] + src[i]
    pub fn batch_add(&mut self, dest: &mut [G1Affine], src: &[G1Affine]) {
        assert!(dest.len() == src.len(), "length of dest and src don't match!");
        assert!(dest.len() <= self.inverses.len(),
                "input length exceeds the max_batch_cnt, please increase max_batch_cnt during initialization!");

        self.reset();
        for i in 0..dest.len() {
            self.batch_add_phase_one(&dest[i], &src[i], i);
        }
        self.inverse();
        for i in (0..dest.len()).rev() {
            self.batch_add_phase_two(&mut dest[i], &src[i], i);
        }
    }

    /// Batch add vector dest and src of len entries, skipping dest_step and src_step entries each
    /// the results will be stored in dest, i.e. dest[i] = dest[i] + src[i]
    pub fn batch_add_step_n(&mut self,
                            dest: &mut [G1Affine],
                            dest_step: usize,
                            src: &[G1Affine],
                            src_step: usize,
                            len: usize) {
        assert!(dest.len() > (len - 1) * dest_step, "insufficient entries in dest array");
        assert!(src.len() > (len - 1) * src_step, "insufficient entries in src array");
        assert!(len <= self.inverses.len(),
                "input length exceeds the max_batch_cnt, please increase max_batch_cnt during initialization!");

        self.reset();
        for i in 0..len {
            self.batch_add_phase_one(&dest[i * dest_step], &src[i * src_step], i);
        }
        self.inverse();
        for i in (0..len).rev() {
            self.batch_add_phase_two(&mut dest[i * dest_step], &src[i * src_step], i);
        }
    }

    pub fn inverse(&mut self) {
        self.inverse_state = self.inverse_state.inverse().unwrap();
    }

    pub fn reset(&mut self) {
        self.inverse_state.set_one();
    }

    /// Two-pass batch affine addition
    ///   - 1st pass calculates from left to right
    ///      - inverse_state: accumulated product of deltaX
    ///      - inverses[]: accumulated product left to a point
    ///   - call inverse()
    ///   - 2nd pass calculates from right to left
    ///      - slope s and ss from state
    ///      - inverse_state = inverse_state * deltaX
    ///      - addition result acc
    /// 以i为界限 设λi = Qxi-Pxi; 
    /// 先计算λi左边，即λ1~i-1连乘结果,不含λi
    /// 再计算λi右边，即λi+1~n的连乘结果，不含λi
    /// 
    pub fn batch_add_phase_one(
            &mut self,
            p: &G1Affine,
            q: &G1Affine,
            idx: usize,
        ) {
        assert!(idx < self.inverses.len(),
                "index exceeds the max_batch_cnt, please increase max_batch_cnt during initialization!");
        if p.is_zero() | q.is_zero() {
            return;
        }

        let mut delta_x = q.x - p.x;
        if delta_x.is_zero() {
            let delta_y = q.y - p.y;
            if !delta_y.is_zero() {
                // p = -q, return
                return;
            }

            // if P == Q  k= 3*(P.x)^2+a /2*p.y
            // if delta_x is zero, we need to invert 2y
            delta_x = q.y + q.y;
        }

        if self.inverse_state.is_zero() {
            self.inverses[idx].set_one();
            self.inverse_state = delta_x;
        } else {
            self.inverses[idx] = self.inverse_state;
            self.inverse_state *= delta_x
        }
    }

    /// should call inverse() between phase_one and phase_two

    pub fn batch_add_phase_two(
            &mut self,
            p: &mut G1Affine,
            q: &G1Affine,
            idx: usize,
        ) {
        assert!(idx < self.inverses.len(),
                "index exceeds the max_batch_cnt, please increase max_batch_cnt during initialization!");
        if p.is_zero() | q.is_zero() {
            if !q.is_zero() {
                *p = q.clone();
            }
            return;
        }

        let mut _inverse = self.inverses[idx];
        _inverse *= self.inverse_state;

        let mut delta_x = q.x - p.x;
        let mut delta_y = q.y - p.y;

        if delta_x.is_zero() {
            if !delta_y.is_zero() {
                // p = -q, result should be pt at infinity
                p.set_zero();
                return;
            }
            // Otherwise, p = q, and it's point doubling
            // Processing is almost the same, except s=3*affine.x^2 / 2*affine.y

            // set delta_y = 3*q.x^2
            delta_y = q.x.square();
            delta_y = delta_y + delta_y + delta_y;

            delta_x = q.y.double();
        }

        // get the state ready for the next iteration
        self.inverse_state *= delta_x;

        let s = delta_y * _inverse;
        let ss = s * s;
        p.x = ss - q.x - p.x;
        delta_x = q.x - p.x;
        p.y = s * delta_x;
        p.y = p.y - q.y;
    }
} 

#[cfg(test)]
mod batch_add_tests {
    use super::*;
    use ark_ec::ProjectiveCurve;
    use ark_std::UniformRand;
    use std::ops::Add;

    #[test]
    fn test_phase_one_zero_or_neg() {
        let mut batch_adder = BatchAdder::new(4);
        batch_adder.batch_add_phase_one(
            &G1Affine::zero(),
            &G1Affine::zero(),
            0
        );

        let mut rng = ark_std::test_rng();
        let p = <G1Affine as AffineCurve>::Projective::rand(&mut rng);
        let p_affine = G1Affine::from(p);
        let mut neg_p_affine = p_affine.clone();
        neg_p_affine.y = -neg_p_affine.y;

        batch_adder.batch_add_phase_one(
            &p_affine,
            &neg_p_affine,
            0
        );
    }

    #[test]
    fn test_phase_one_p_add_p() {
        let mut batch_adder = BatchAdder::new(4);
        let mut rng = ark_std::test_rng();
        let prj = <G1Affine as AffineCurve>::Projective::rand(&mut rng);
        let p = G1Affine::from(prj);
        let acc = p.clone();

        batch_adder.batch_add_phase_one(&acc, &p, 0);
        assert_eq!(batch_adder.inverses[0].is_one(), true);
        assert_eq!(batch_adder.inverse_state, p.y + p.y);
    }

    #[test]
    fn test_phase_one_p_add_q() {
        let mut batch_adder = BatchAdder::new(4);
        let mut rng = ark_std::test_rng();
        let p_prj = <G1Affine as AffineCurve>::Projective::rand(&mut rng);
        let q_prj = <G1Affine as AffineCurve>::Projective::rand(&mut rng);
        let p = G1Affine::from(p_prj);
        let q = G1Affine::from(q_prj);

        batch_adder.batch_add_phase_one(&p, &q, 0);
        assert_eq!(batch_adder.inverses[0].is_one(), true);
        assert_eq!(batch_adder.inverse_state, q.x - p.x);
    }

    #[test]
    fn test_phase_one_p_add_q_twice() {
        let mut batch_adder = BatchAdder::new(4);
        let mut rng = ark_std::test_rng();
        let p_prj = <G1Affine as AffineCurve>::Projective::rand(&mut rng);
        let q_prj = <G1Affine as AffineCurve>::Projective::rand(&mut rng);
        let p = G1Affine::from(p_prj);
        let q = G1Affine::from(q_prj);

        batch_adder.batch_add_phase_one(&p, &q, 0);
        batch_adder.batch_add_phase_one(&p, &q, 0);
        assert_eq!(batch_adder.inverses[0], q.x - p.x);
        assert_eq!(batch_adder.inverse_state, (q.x - p.x) * (q.x - p.x));
    }

    #[test]
    fn test_phase_two_zero_add_p() {
        let mut batch_adder = BatchAdder::new(4);
        let mut rng = ark_std::test_rng();
        let p_prj = <G1Affine as AffineCurve>::Projective::rand(&mut rng);
        let p = G1Affine::from(p_prj);
        let mut acc = G1Affine::zero();
        batch_adder.batch_add_phase_two(&mut acc, &p, 0);
        assert_eq!(acc, p);
    }

    #[test]
    fn test_phase_two_p_add_neg() {
        let mut batch_adder = BatchAdder::new(4);

        let mut rng = ark_std::test_rng();
        let p_prj = <G1Affine as AffineCurve>::Projective::rand(&mut rng);
        let mut acc = G1Affine::from(p_prj);
        let mut p = acc.clone();
        p.y = -p.y;

        batch_adder.batch_add_phase_two(&mut acc, &p, 0);
        assert_eq!(acc, G1Affine::zero());
    }

    #[test]
    fn test_phase_two_p_add_q() {
        let mut batch_adder = BatchAdder::new(4);

        let mut rng = ark_std::test_rng();
        let acc_prj = <G1Affine as AffineCurve>::Projective::rand(&mut rng);
        let mut acc = G1Affine::from(acc_prj);
        let mut p = acc.clone();
        p.x = p.x + p.x;

        batch_adder.inverses[0] = (p.x - acc.x).inverse().unwrap();
        batch_adder.batch_add_phase_two(&mut acc, &p, 0);
        assert_eq!(acc, G1Affine::from(acc_prj.add_mixed(&p)));
    }

    #[test]
    fn test_phase_two_p_add_p() {
        let mut batch_adder = BatchAdder::new(4);

        let mut rng = ark_std::test_rng();
        let acc_prj = <G1Affine as AffineCurve>::Projective::rand(&mut rng);
        let mut acc = G1Affine::from(acc_prj);
        let p = acc.clone();

        batch_adder.inverses[0] = (p.y + p.y).inverse().unwrap();
        batch_adder.batch_add_phase_two(&mut acc, &p, 0);
        assert_eq!(acc, G1Affine::from(acc_prj).add(p));
    }

    #[test]
    fn test_batch_add() {
        let mut batch_adder = BatchAdder::new(10);

        let mut rng = ark_std::test_rng();
        let mut buckets: Vec<G1Affine> = (0..10)
            .map(|_| G1Affine::from(<G1Affine as AffineCurve>::Projective::rand(&mut rng)))
            .collect();
        let points: Vec<G1Affine> = (0..10)
            .map(|_| G1Affine::from(<G1Affine as AffineCurve>::Projective::rand(&mut rng)))
            .collect();

        let tmp = buckets.clone();
        batch_adder.batch_add(&mut buckets, &points);

        for i in 0..10 {
            assert_eq!(buckets[i], tmp[i].add(points[i]));
        }
    }

    #[test]
    fn test_batch_add_step_n() {
        let mut batch_adder = BatchAdder::new(10);

        let mut rng = ark_std::test_rng();
        let mut buckets: Vec<G1Affine> = (0..10)
            .map(|_| G1Affine::from(<G1Affine as AffineCurve>::Projective::rand(&mut rng)))
            .collect();
        let points: Vec<G1Affine> = (0..10)
            .map(|_| G1Affine::from(<G1Affine as AffineCurve>::Projective::rand(&mut rng)))
            .collect();

        let tmp = buckets.clone();
        batch_adder.batch_add_step_n(&mut buckets, 1, &points, 2, 3);

        for i in 0..3 {
            assert_eq!(buckets[i], tmp[i].add(points[i * 2]));
        }
    }
}